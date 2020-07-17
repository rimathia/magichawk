extern crate chrono;
#[macro_use]
extern crate lazy_static;
extern crate log;
extern crate pdfgen_bindings;
extern crate regex;
extern crate reqwest;

use image::{
    imageops::overlay,
    jpeg::{JPEGEncoder, PixelDensity, PixelDensityUnit},
    DynamicImage, GenericImageView, Rgba, RgbaImage,
};
use log::{debug, error, info};
use regex::Match;
use regex::Regex;
use std::io::{Cursor, Read};
use std::os::unix::ffi::OsStrExt;
use std::string::String;
use std::time::{Duration, Instant};
use Option::{None, Some};

pub const IMAGE_WIDTH: u32 = 480;
pub const IMAGE_HEIGHT: u32 = 680;

pub const PAGE_WIDTH: u32 = 3 * IMAGE_WIDTH;
pub const PAGE_HEIGHT: u32 = 3 * IMAGE_HEIGHT;

pub const IMAGE_HEIGHT_CM: f64 = 8.7;
pub const IMAGE_WIDTH_CM: f64 = IMAGE_HEIGHT_CM * IMAGE_WIDTH as f64 / IMAGE_HEIGHT as f64;

pub fn setup_logger() -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Utc::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Debug)
        .chain(std::io::stdout())
        .chain(fern::log_file("output.log")?)
        .apply()?;
    Ok(())
}

#[derive(Debug, PartialEq)]
pub struct DecklistEntry<'a> {
    multiple: i32,
    name: &'a str,
    set: Option<&'a str>,
}

#[derive(Debug, PartialEq)]
pub struct ParsedDecklistLine<'a> {
    line: &'a str,
    entry: Option<DecklistEntry<'a>>,
}

fn parse_set(group: Option<Match>) -> Option<&str> {
    Some(group?.as_str())
}

fn parse_multiple(group: Option<Match>) -> i32 {
    match group {
        Some(m) => m.as_str().parse().ok().unwrap_or(1),
        None => 1,
    }
}

pub fn parse_line(line: &str) -> Option<DecklistEntry> {
    lazy_static! {
        static ref REMNS: Regex =
            Regex::new(r"^\s*(\d*)\s*([^\(\[\$\t]*)[\s\(\[]*([\dA-Z]{3})?").unwrap();
    }

    match REMNS.captures(line) {
        Some(mns) => Some(DecklistEntry {
            multiple: parse_multiple(mns.get(1)),
            name: mns.get(2)?.as_str().trim(),
            set: parse_set(mns.get(3)),
        }),
        None => None,
    }
}

pub fn parse_decklist(decklist: &str) -> Vec<ParsedDecklistLine> {
    decklist
        .lines()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| ParsedDecklistLine {
            line: s,
            entry: parse_line(s),
        })
        .collect()
}

pub fn encode_card_name(name: &str) -> String {
    name.replace(" ", "+")
}

pub fn query_image(name: &str, set: Option<&str>) -> Option<DynamicImage> {
    let mut url = format!(
        "https://api.scryfall.com/cards/named?fuzzy={}&version=border_crop&format=image",
        encode_card_name(name)
    );
    if set.is_some() {
        url += format!("&set={}", set.unwrap()).as_str();
    }

    debug!("scryfall uri: {}", url);

    let request = reqwest::blocking::get(&url);
    match request {
        Ok(reqok) => match reqok.bytes() {
            Ok(b) => match image::load_from_memory_with_format(&b, image::ImageFormat::Jpeg) {
                Ok(im) => {
                    return Some(im);
                }
                Err(e) => {
                    error!("error converting response to jpeg: {}", e);
                    return None;
                }
            },
            Err(e) => {
                info!("error in getting bytes of image: {}", e);
                return None;
            }
        },
        Err(e) => {
            info!("error in image request: {}", e);
            return None;
        }
    }
}

pub struct ScryfallCache {
    last_query: Instant,
    images: std::collections::HashMap<(String, Option<String>), Option<DynamicImage>>,
}

impl ScryfallCache {
    const COOLDOWN: Duration = Duration::from_millis(100);

    pub fn new() -> ScryfallCache {
        ScryfallCache {
            last_query: Instant::now(),
            images: std::collections::HashMap::new(),
        }
    }

    fn query_image(&mut self, name: &str, set: Option<&str>) -> &Option<DynamicImage> {
        let key = (name.to_string(), set.map(|s| s.to_string()));
        if self.images.contains_key(&key) {
            debug!("image for name {:?} and set {:?} is cached", name, set);
        } else {
            let n = Instant::now();
            if n - self.last_query < ScryfallCache::COOLDOWN {
                debug!("waiting before next scryfall call");
                std::thread::sleep(ScryfallCache::COOLDOWN - (n - self.last_query));
            } else {
                debug!("last scryfall call was {:?} ago", n - self.last_query);
            }
            self.last_query = n;
            self.images.insert(key.clone(), query_image(name, set));
        }
        return self.images.get(&key).unwrap();
    }

    pub fn list(&self) -> String {
        let mut desc: String = "<ul>".to_string();
        for (key, value) in &self.images {
            desc.push_str(
                format!(
                    "<li>{:?}: {}</li>",
                    key,
                    match value {
                        Some(_) => "cached",
                        None => "query failed",
                    },
                )
                .as_str(),
            );
        }
        desc.push_str("</ul>");
        desc
    }
}

pub fn images_to_pdf(images: Vec<DynamicImage>) -> Option<Vec<u8>> {
    if images.iter().any(|i| {
        let dim = i.dimensions();
        if dim != (IMAGE_WIDTH, IMAGE_HEIGHT) {
            error!("unexpected image dimensions {:?}", dim);
            return true;
        }
        return false;
    }) {
        return None;
    }

    if images.len() == 0 {
        error!("empty vector of images");
        return None;
    }

    let mut pos_hor = 0;
    let mut pos_ver = 0;

    let white_pixel = Rgba::<u8>([255, 255, 255, 255]);
    let mut composed = RgbaImage::from_pixel(0, 0, white_pixel);

    let info = pdfgen_bindings::PDFInfo {
        creator: [0; 64],
        producer: [0; 64],
        title: [0; 64],
        author: [0; 64],
        subject: [0; 64],
        date: [0; 64],
    };

    let pdf = unsafe {
        pdfgen_bindings::pdf_create(pdfgen_bindings::A4_WIDTH, pdfgen_bindings::A4_HEIGHT, &info)
    };

    for (i_im, im) in images.iter().enumerate() {
        let mut outputbuffer: Vec<u8> = vec![];
        let mut outputcursor = Cursor::new(&mut outputbuffer);
        let mut encoder = JPEGEncoder::new_with_quality(&mut outputcursor, 100);
        let image_px_per_cm: u16 = ((IMAGE_HEIGHT as f64) / IMAGE_HEIGHT_CM).round() as u16;

        let image_dpi: PixelDensity = PixelDensity {
            density: (image_px_per_cm, image_px_per_cm),
            unit: PixelDensityUnit::Centimeters,
        };
        encoder.set_pixel_density(image_dpi);
        if pos_hor == 0 && pos_ver == 0 {
            composed = RgbaImage::from_pixel(PAGE_WIDTH, PAGE_HEIGHT, white_pixel);
        }
        overlay(
            &mut composed,
            im,
            pos_hor * IMAGE_WIDTH,
            pos_ver * IMAGE_HEIGHT,
        );
        pos_hor += 1;
        if pos_hor == 3 {
            pos_hor = 0;
            pos_ver += 1;
            if pos_ver == 3 {
                pos_ver = 0;
            }
        }
        if i_im % 9 == 8 || i_im == images.len() - 1 {
            match encoder.encode_image(&composed) {
                Ok(_) => unsafe {
                    let page = pdfgen_bindings::pdf_append_page(pdf);
                    pdfgen_bindings::pdf_add_jpeg_data(
                        pdf,
                        page,
                        pdfgen_bindings::mm_to_point(13.0),
                        pdfgen_bindings::mm_to_point(18.0),
                        pdfgen_bindings::mm_to_point((3.0f64 * IMAGE_WIDTH_CM * 10.0f64) as f32),
                        pdfgen_bindings::mm_to_point((3.0f64 * IMAGE_HEIGHT_CM * 10.0) as f32),
                        outputbuffer.as_slice().as_ptr(),
                        outputbuffer.len(),
                    );
                },
                Err(e) => {
                    error!("error in jpeg encoding: {}", e);
                    return None;
                }
            }
        }
    }

    let tmpfilename = match tempfile::NamedTempFile::new() {
        Ok(t) => t.into_temp_path(),
        Err(e) => {
            error!("error in creation of temporary file: {}", e);
            return None;
        }
    };
    debug!("created temporary file {:?}", tmpfilename);
    let tmpfilename_c = match std::ffi::CString::new(tmpfilename.as_os_str().as_bytes()) {
        Ok(x) => x,
        Err(e) => {
            error!("error in creating c string: {}", e);
            return None;
        }
    };

    unsafe {
        pdfgen_bindings::pdf_save(pdf, tmpfilename_c.as_ptr());
        pdfgen_bindings::pdf_destroy(pdf);
    }

    let mut pdfbytes = Vec::<u8>::new();
    std::fs::File::open(tmpfilename)
        .unwrap()
        .read_to_end(&mut pdfbytes)
        .unwrap();

    Some(pdfbytes)
}

pub fn decklist_to_images(cache: &mut ScryfallCache, decklist: &str) -> Option<Vec<DynamicImage>> {
    let parsed = parse_decklist(decklist);
    let mut images: Vec<DynamicImage> = vec![];
    for line in parsed.iter() {
        match &line.entry {
            Some(e) => {
                let im = cache.query_image(e.name, e.set);
                match im {
                    Some(i) => {
                        debug!("adding image for line {:?}", line);
                        for _ in 0..e.multiple {
                            images.push(i.clone());
                        }
                    }
                    None => {}
                }
            }
            None => {}
        }
    }
    Some(images)
}

pub fn decklist_to_pdf(cache: &mut ScryfallCache, decklist: &str) -> Option<Vec<u8>> {
    images_to_pdf(decklist_to_images(cache, decklist)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn name() {
        assert_eq!(
            parse_line("plains").unwrap(),
            DecklistEntry {
                multiple: 1,
                name: "plains",
                set: None
            }
        );
    }

    #[test]
    fn number_name() {
        assert_eq!(
            parse_line("2\tplains").unwrap(),
            DecklistEntry {
                multiple: 2,
                name: "plains",
                set: None
            }
        );
    }

    #[test]
    fn number_name_set() {
        assert_eq!(
            parse_line("17 long card's name [IPA]").unwrap(),
            DecklistEntry {
                multiple: 17,
                name: "long card's name",
                set: Some("IPA")
            }
        );
    }

    #[test]
    fn name_set() {
        assert_eq!(
            parse_line("long card's name [IPA]").unwrap(),
            DecklistEntry {
                multiple: 1,
                name: "long card's name",
                set: Some("IPA")
            }
        );
    }

    #[test]
    fn name_with_tab() {
        assert_eq!(
            parse_line("Incubation/Incongruity   \t\t---").unwrap(),
            DecklistEntry {
                multiple: 1,
                name: "Incubation/Incongruity",
                set: None
            }
        );
    }

    #[test]
    fn mtgdecks() {
        let decklist = "4  Beanstalk Giant   		$0.25
        4  Lovestruck Beast   		$1.5
        Artifact [5]
        1  The Great Henge   		$25
        Instant [1]
        1  Incubation/Incongruity   		--- ";
        let parsed = parse_decklist(decklist);
        let expected = vec![
            ParsedDecklistLine {
                line: "4  Beanstalk Giant   		$0.25",
                entry: Some(DecklistEntry {
                    multiple: 4,
                    name: "Beanstalk Giant",
                    set: None,
                }),
            },
            ParsedDecklistLine {
                line: "4  Lovestruck Beast   		$1.5",
                entry: Some(DecklistEntry {
                    multiple: 4,
                    name: "Lovestruck Beast",
                    set: None,
                }),
            },
            ParsedDecklistLine {
                line: "Artifact [5]",
                entry: Some(DecklistEntry {
                    multiple: 1,
                    name: "Artifact",
                    set: None,
                }),
            },
            ParsedDecklistLine {
                line: "1  The Great Henge   		$25",
                entry: Some(DecklistEntry {
                    multiple: 1,
                    name: "The Great Henge",
                    set: None,
                }),
            },
            ParsedDecklistLine {
                line: "Instant [1]",
                entry: Some(DecklistEntry {
                    multiple: 1,
                    name: "Instant",
                    set: None,
                }),
            },
            ParsedDecklistLine {
                line: "1  Incubation/Incongruity   		---",
                entry: Some(DecklistEntry {
                    multiple: 1,
                    name: "Incubation/Incongruity",
                    set: None,
                }),
            },
        ];
        for (left, right) in parsed.iter().zip(expected.iter()) {
            assert_eq!(left, right);
        }
    }

    #[test]
    fn arenaexport() {
        let decklist = "Deck
        1 Bedeck // Bedazzle (RNA) 221
        1 Spawn of Mayhem (RNA) 85
        ";
        let expected = vec![
            ParsedDecklistLine {
                line: "Deck",
                entry: Some(DecklistEntry {
                    multiple: 1,
                    name: "Deck",
                    set: None,
                }),
            },
            ParsedDecklistLine {
                line: "1 Bedeck // Bedazzle (RNA) 221",
                entry: Some(DecklistEntry {
                    multiple: 1,
                    name: "Bedeck // Bedazzle",
                    set: Some("RNA"),
                }),
            },
            ParsedDecklistLine {
                line: "1 Spawn of Mayhem (RNA) 85",
                entry: Some(DecklistEntry {
                    multiple: 1,
                    name: "Spawn of Mayhem",
                    set: Some("RNA"),
                }),
            },
        ];
        let parsed = parse_decklist(decklist);
        for (left, right) in parsed.iter().zip(expected.iter()) {
            assert_eq!(left, right);
        }
    }
}
