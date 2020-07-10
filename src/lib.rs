extern crate chrono;
#[macro_use]
extern crate lazy_static;
extern crate log;
extern crate regex;
extern crate reqwest;

use image::{imageops::overlay, DynamicImage, GenericImageView, Rgba, RgbaImage};
use log::{debug, error, info};
use regex::Match;
use regex::Regex;
use std::io::{Cursor, Write};
use std::string::String;
use std::time::{Duration, Instant};
use tiff::{
    encoder::{colortype::RGBA8, Rational, TiffEncoder},
    tags::ResolutionUnit,
};
use Option::{None, Some};

pub const IMAGE_WIDTH: u32 = 480;
pub const IMAGE_HEIGHT: u32 = 680;

pub const PAGE_WIDTH: u32 = 3 * IMAGE_WIDTH;
pub const PAGE_HEIGHT: u32 = 3 * IMAGE_HEIGHT;

pub const IMAGE_HEIGHT_CM: f64 = 8.7;

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
}

pub fn create_tiff(images: Vec<DynamicImage>) -> Option<Vec<u8>> {
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

    let mut outputbuffer: Vec<u8> = vec![];
    let mut f = Cursor::new(&mut outputbuffer);
    let mut encoder = TiffEncoder::new(&mut f).unwrap();

    let mut pos_hor = 0;
    let mut pos_ver = 0;

    let white_pixel = Rgba::<u8>([255, 255, 255, 255]);
    let mut composed = RgbaImage::from_pixel(0, 0, white_pixel);

    for (i_im, im) in images.iter().enumerate() {
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
            let mut idx = 0;
            match encoder.new_image::<RGBA8>(PAGE_WIDTH, PAGE_HEIGHT) {
                Ok(mut page) => {
                    page.resolution(
                        ResolutionUnit::Centimeter,
                        Rational {
                            n: ((IMAGE_HEIGHT as f64) / IMAGE_HEIGHT_CM).round() as u32,
                            d: 1,
                        },
                    );
                    let buffer = composed.clone().into_raw();
                    while page.next_strip_sample_count() > 0 {
                        let sample_count = page.next_strip_sample_count() as usize;
                        match page.write_strip(&buffer[idx..idx + sample_count]) {
                            Ok(_) => {}
                            Err(e) => {
                                error!("error in writing tiff strip: {}", e);
                                return None;
                            }
                        }
                        idx += sample_count;
                    }
                    match page.finish() {
                        Ok(_) => {}
                        Err(e) => {
                            error!("error in finishing tiff page: {}", e);
                            return None;
                        }
                    }
                }
                Err(e) => {
                    error!("tiff error in page creation: {}", e);
                    return None;
                }
            }
        }
    }
    debug!("output buffer has size {}", outputbuffer.len());
    Some(outputbuffer)
}

pub fn decklist_to_tiff(cache: &mut ScryfallCache, decklist: &str) -> Option<Vec<u8>> {
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
    create_tiff(images)
}

pub fn decklist_to_zipped_tiff(cache: &mut ScryfallCache, decklist: &str) -> Option<Vec<u8>> {
    let t = decklist_to_tiff(cache, decklist)?;
    debug!("tiff file has size {}", t.len());
    let zipbuffer: Vec<u8> = vec![];
    let mut zip = zip::ZipWriter::new(Cursor::new(zipbuffer));
    match zip.start_file("proxies.tiff", zip::write::FileOptions::default()) {
        Err(e) => {
            error!("error in starting file: {}", e);
            return None;
        }
        Ok(_) => {}
    }
    match zip.write_all(&t) {
        Err(e) => {
            error!("error in writing to zip file: {}", e);
            return None;
        }
        Ok(_) => {}
    }
    match zip.finish() {
        Ok(w) => {
            let zipped_bytes = w.into_inner();
            debug!("zipped file has size {}", zipped_bytes.len());
            Some(zipped_bytes)
        }
        Err(e) => {
            error!("error in creating zip from tiff: {}", e);
            None
        }
    }
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
}
