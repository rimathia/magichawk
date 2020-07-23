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
    DynamicImage, GenericImage, GenericImageView, ImageResult, Rgba, RgbaImage,
};
use itertools::Itertools;
use log::{debug, error, info};
use regex::Match;
use regex::Regex;
use std::convert::TryFrom;
use std::fmt;
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

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Card {
    name: String,
    set: Option<String>,
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.set {
            Some(set) => write!(f, "{} ({})", self.name, set),
            None => write!(f, "{}", self.name),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct DecklistEntry {
    multiple: i32,
    card: Card,
}

impl DecklistEntry {
    pub fn new(m: i32, n: &str, s: Option<&str>) -> DecklistEntry {
        DecklistEntry {
            multiple: m,
            card: Card {
                name: n.to_string(),
                set: s.map(|x| x.to_string()),
            },
        }
    }

    pub fn from_name(n: &str) -> DecklistEntry {
        DecklistEntry {
            multiple: 1,
            card: Card {
                name: n.to_string(),
                set: None,
            },
        }
    }

    pub fn from_multiple_name(m: i32, n: &str) -> DecklistEntry {
        DecklistEntry {
            multiple: m,
            card: Card {
                name: n.to_string(),
                set: None,
            },
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct ParsedDecklistLine<'a> {
    line: &'a str,
    entry: Option<DecklistEntry>,
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
            card: Card {
                name: mns.get(2)?.as_str().trim().to_string(),
                set: parse_set(mns.get(3)).map(|s| s.to_string()),
            },
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

pub fn query_image(card: &Card) -> Option<DynamicImage> {
    let mut url = format!(
        "https://api.scryfall.com/cards/named?fuzzy={}&version=border_crop&format=image",
        encode_card_name(card.name.as_str())
    );
    if card.set.is_some() {
        url += format!("&set={}", card.set.as_ref().unwrap()).as_str();
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

pub struct CachedImageResponse {
    t: Instant,
    image: Option<DynamicImage>,
}

impl CachedImageResponse {
    pub fn from_image(i: DynamicImage) -> CachedImageResponse {
        CachedImageResponse {
            t: Instant::now(),
            image: Some(i),
        }
    }
}

impl fmt::Display for CachedImageResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let age = Instant::now() - self.t;
        match self.image {
            Some(_) => write!(f, "created {:?} ago, contains image", age),
            None => write!(f, "created {:?} ago, no image", age),
        }
    }
}

impl fmt::Debug for CachedImageResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

pub struct ScryfallCache {
    last_query: Instant,
    last_purge: Instant,
    images: std::collections::HashMap<Card, CachedImageResponse>,
}

impl ScryfallCache {
    const COOLDOWN: Duration = Duration::from_millis(100);

    const MAX_AGE: Duration = Duration::from_secs(14 * 24 * 60 * 60);

    pub fn new() -> ScryfallCache {
        ScryfallCache {
            last_query: Instant::now(),
            last_purge: Instant::now(),
            images: std::collections::HashMap::new(),
        }
    }

    fn ensure_contains(&mut self, card: &Card) {
        let n = Instant::now();
        if n - self.last_purge > ScryfallCache::MAX_AGE {
            self.purge(Some(ScryfallCache::MAX_AGE));
        }
        if self.images.contains_key(card) {
            debug!("image for {} is cached", card);
        } else {
            if n - self.last_query < ScryfallCache::COOLDOWN {
                debug!("waiting before next scryfall call");
                std::thread::sleep(ScryfallCache::COOLDOWN - (n - self.last_query));
            } else {
                debug!("last scryfall call was {:?} ago", n - self.last_query);
            }
            self.last_query = n;
            self.images.insert(
                card.clone(),
                CachedImageResponse {
                    t: Instant::now(),
                    image: query_image(card),
                },
            );
        }
    }

    fn query_image(&mut self, card: &Card) -> Option<DynamicImage> {
        self.ensure_contains(card);
        self.images.get(&card).unwrap().image.clone()
    }

    pub fn list(&self) -> String {
        let mut desc: String = "<ul>".to_string();
        for (key, value) in &self.images {
            desc.push_str(format!("<li>{}: {:?}</li>", key, value).as_str());
        }
        desc.push_str("</ul>");
        desc
    }

    pub fn purge(&mut self, max_age: Option<Duration>) {
        let n = Instant::now();
        debug!("{} cached responses before purging", self.images.len());
        self.images
            .retain(|_, value| n - value.t < max_age.unwrap_or(ScryfallCache::MAX_AGE));
        self.last_purge = n;
        debug!("{} cached responses after purging", self.images.len());
    }
}

pub fn expand_multiples(entry: DecklistEntry) -> itertools::RepeatN<Card> {
    itertools::repeat_n(entry.card, usize::try_from(entry.multiple).unwrap_or(0))
}

pub fn images_to_page<I>(mut it: I) -> Option<RgbaImage>
where
    I: Iterator<Item = DynamicImage>,
{
    let mut pos_hor = 0;
    let mut pos_ver = 0;

    let mut composed: Option<RgbaImage> = None;
    let white_pixel = Rgba::<u8>([255, 255, 255, 255]);

    loop {
        match it.next() {
            None => return composed,
            Some(im) => {
                overlay(
                    composed.get_or_insert(RgbaImage::from_pixel(
                        PAGE_WIDTH,
                        PAGE_HEIGHT,
                        white_pixel,
                    )),
                    &im,
                    pos_hor * IMAGE_WIDTH,
                    pos_ver * IMAGE_HEIGHT,
                );
                pos_hor += 1;
                if pos_hor == 3 {
                    pos_hor = 0;
                    pos_ver += 1;
                }
                if pos_ver == 3 {
                    return composed;
                }
            }
        }
    }
}

pub fn encode_jpeg<I: GenericImageView>(im: &I) -> ImageResult<Vec<u8>> {
    let mut outputbuffer: Vec<u8> = vec![];
    let mut outputcursor = Cursor::new(&mut outputbuffer);
    let mut encoder = JPEGEncoder::new_with_quality(&mut outputcursor, 100);
    let image_px_per_cm: u16 = ((IMAGE_HEIGHT as f64) / IMAGE_HEIGHT_CM).round() as u16;

    let image_dpi: PixelDensity = PixelDensity {
        density: (image_px_per_cm, image_px_per_cm),
        unit: PixelDensityUnit::Centimeters,
    };
    encoder.set_pixel_density(image_dpi);
    encoder.encode_image(im)?;
    Ok(outputbuffer)
}

pub fn pages_to_pdf<I>(mut it: I) -> Option<Vec<u8>>
where
    I: Iterator,
    I::Item: GenericImage,
{
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

    loop {
        match it.next() {
            Some(grid) => match encode_jpeg(&grid) {
                Ok(outputbuffer) => unsafe {
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
            },
            None => {
                let tmpfilename = match tempfile::NamedTempFile::new() {
                    Ok(t) => t.into_temp_path(),
                    Err(e) => {
                        error!("error in creation of temporary file: {}", e);
                        return None;
                    }
                };
                debug!("created temporary file {:?}", tmpfilename);
                let tmpfilename_c = match std::ffi::CString::new(tmpfilename.as_os_str().as_bytes())
                {
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

                return Some(pdfbytes);
            }
        }
    }
}

pub fn decklist_to_pdf(cache: &mut ScryfallCache, decklist: &str) -> Option<Vec<u8>> {
    pages_to_pdf(
        parse_decklist(decklist)
            .into_iter()
            .flat_map(|parsed| parsed.entry.into_iter())
            .flat_map(expand_multiples)
            .map(|it| cache.query_image(&it))
            .flat_map(|e| e.into_iter())
            .batching(|it| images_to_page(it)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn name() {
        assert_eq!(
            parse_line("plains").unwrap(),
            DecklistEntry::from_name("plains")
        );
    }

    #[test]
    fn number_name() {
        assert_eq!(
            parse_line("2\tplains").unwrap(),
            DecklistEntry::from_multiple_name(2, "plains")
        );
    }

    #[test]
    fn number_name_set() {
        assert_eq!(
            parse_line("17 long card's name [IPA]").unwrap(),
            DecklistEntry::new(17, "long card's name", Some("IPA"))
        );
    }

    #[test]
    fn name_set() {
        assert_eq!(
            parse_line("long card's name [IPA]").unwrap(),
            DecklistEntry::new(1, "long card's name", Some("IPA"))
        );
    }

    #[test]
    fn name_with_tab() {
        assert_eq!(
            parse_line("Incubation/Incongruity   \t\t---").unwrap(),
            DecklistEntry::from_multiple_name(1, "Incubation/Incongruity")
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
                entry: Some(DecklistEntry::from_multiple_name(4, "Beanstalk Giant")),
            },
            ParsedDecklistLine {
                line: "4  Lovestruck Beast   		$1.5",
                entry: Some(DecklistEntry::from_multiple_name(4, "Lovestruck Beast")),
            },
            ParsedDecklistLine {
                line: "Artifact [5]",
                entry: Some(DecklistEntry::from_multiple_name(1, "Artifact")),
            },
            ParsedDecklistLine {
                line: "1  The Great Henge   		$25",
                entry: Some(DecklistEntry::from_multiple_name(1, "The Great Henge")),
            },
            ParsedDecklistLine {
                line: "Instant [1]",
                entry: Some(DecklistEntry::from_multiple_name(1, "Instant")),
            },
            ParsedDecklistLine {
                line: "1  Incubation/Incongruity   		---",
                entry: Some(DecklistEntry::from_multiple_name(
                    1,
                    "Incubation/Incongruity",
                )),
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
                entry: Some(DecklistEntry::from_multiple_name(1, "Deck")),
            },
            ParsedDecklistLine {
                line: "1 Bedeck // Bedazzle (RNA) 221",
                entry: Some(DecklistEntry::new(1, "Bedeck // Bedazzle", Some("RNA"))),
            },
            ParsedDecklistLine {
                line: "1 Spawn of Mayhem (RNA) 85",
                entry: Some(DecklistEntry::new(1, "Spawn of Mayhem", Some("RNA"))),
            },
        ];
        let parsed = parse_decklist(decklist);
        for (left, right) in parsed.iter().zip(expected.iter()) {
            assert_eq!(left, right);
        }
    }
}
