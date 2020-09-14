extern crate chrono;
#[macro_use]
extern crate lazy_static;
extern crate log;
extern crate pdfgen_bindings;
extern crate regex;
extern crate reqwest;
extern crate rocket;
extern crate serde;
extern crate serde_json;

use chrono::{DateTime, Utc};
use image::{
    imageops::overlay,
    jpeg::{JPEGEncoder, PixelDensity, PixelDensityUnit},
    DynamicImage, GenericImage, GenericImageView, ImageResult, Rgba, RgbaImage,
};
use log::{debug, error, info};
use ngrammatic::*;
use regex::Match;
use regex::Regex;
use rocket::{http::RawStr, request::FromFormValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{
    hash_map::Entry::{Occupied, Vacant},
    HashMap,
};
use std::convert::TryFrom;
use std::fmt;
use std::io::{Cursor, Read};
use std::os::unix::ffi::OsStrExt;
use std::string::String;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use Option::{None, Some};

pub const SCRYFALL_CARD_NAMES: &'static str = "https://api.scryfall.com/catalog/card-names";

pub const IMAGE_WIDTH: u32 = 480;
pub const IMAGE_HEIGHT: u32 = 680;

pub const PAGE_WIDTH: u32 = 3 * IMAGE_WIDTH;
pub const PAGE_HEIGHT: u32 = 3 * IMAGE_HEIGHT;

pub const IMAGE_HEIGHT_CM: f64 = 8.7;
pub const IMAGE_WIDTH_CM: f64 = IMAGE_HEIGHT_CM * IMAGE_WIDTH as f64 / IMAGE_HEIGHT as f64;

pub const SCRYFALL_COOLDOWN: Duration = Duration::from_millis(100);
lazy_static! {
    static ref LAST_SCRYFALL_CALL: Mutex<Instant> = Mutex::new(Instant::now() - SCRYFALL_COOLDOWN);
}

pub fn scryfall_call(uri: &str) -> reqwest::Result<reqwest::blocking::Response> {
    let mut last_call = LAST_SCRYFALL_CALL.lock().unwrap();
    let mut n = Instant::now();
    if n - *last_call < SCRYFALL_COOLDOWN {
        debug!("waiting before next scryfall call");
        std::thread::sleep(SCRYFALL_COOLDOWN - (n - *last_call));
    } else {
        debug!("last scryfall call was {:?} ago", n - *last_call);
        n = Instant::now();
    }
    *last_call = n;
    reqwest::blocking::get(uri)
}

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

#[derive(Serialize, Deserialize)]
pub struct ScryfallCardNames {
    pub object: String,
    pub uri: String,
    pub total_values: i32,
    #[serde(alias = "data")]
    pub names: Vec<String>,
}

impl ScryfallCardNames {
    pub fn from_api_call() -> Option<ScryfallCardNames> {
        let mut card_names: ScryfallCardNames =
            serde_json::from_reader(scryfall_call(SCRYFALL_CARD_NAMES).ok()?).ok()?;
        for name in card_names.names.iter_mut() {
            *name = name.to_lowercase();
        }
        return Some(card_names);
    }
}

#[derive(Serialize, Deserialize)]
pub struct ScryfallSearchAnswer {
    pub object: String,
    pub total_cards: i32,
    pub has_more: bool,
    pub next_page: Option<String>,
    pub data: Vec<serde_json::Map<String, Value>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CardPrinting {
    pub set: String,
    pub border_crop: String,
    pub border_crop_back: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ScryfallCard {
    pub name: String,
    pub printing: CardPrinting,
}

impl ScryfallCard {
    pub fn from_scryfall_object(d: &serde_json::Map<String, Value>) -> Option<ScryfallCard> {
        let n: String = d["name"].as_str()?.to_string().to_lowercase();
        let s = d["set"].as_str()?.to_string().to_lowercase();
        let (bc, bcb) = {
            if d.contains_key("image_uris") {
                (d["image_uris"]["border_crop"].as_str()?.to_string(), None)
            } else if d.contains_key("card_faces") {
                let card_faces = d["card_faces"].as_array()?;
                if card_faces.len() != 2 {
                    return None;
                } else {
                    (
                        card_faces[0]["image_uris"]["border_crop"]
                            .as_str()?
                            .to_string(),
                        Some(
                            card_faces[1]["image_uris"]["border_crop"]
                                .as_str()?
                                .to_string(),
                        ),
                    )
                }
            } else {
                return None;
            }
        };
        return Some(ScryfallCard {
            name: n,
            printing: CardPrinting {
                set: s,
                border_crop: bc,
                border_crop_back: bcb,
            },
        });
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Card {
    pub name: String,
    pub set: Option<String>, // TODO: private
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.set {
            Some(set) => write!(f, "{} ({})", self.name, set),
            None => write!(f, "{}", self.name),
        }
    }
}

pub fn insert_scryfall_card(
    printings: &mut HashMap<String, Vec<CardPrinting>>,
    card_names: &ScryfallCardNames,
    card: ScryfallCard,
) {
    if card_names.names.contains(&card.name.to_lowercase()) {
        printings
            .entry(card.name.to_lowercase())
            .or_insert(Vec::new())
            .push(card.printing);
    } else {
        error!(
            "couldn't insert scryfall card because name was unknown: {:?}",
            card
        )
    }
}

pub fn insert_scryfall_object(
    printings: &mut HashMap<String, Vec<CardPrinting>>,
    card_names: &ScryfallCardNames,
    object: &serde_json::Map<String, Value>,
) {
    match ScryfallCard::from_scryfall_object(object) {
        Some(card) => insert_scryfall_card(printings, card_names, card),
        None => error!("couldn't convert scryfall object {:?}", object),
    }
}

pub struct CardData {
    pub card_names: ScryfallCardNames,                 // TODO(private)
    pub lookup: CardNameLookup,                        // TODO(private)
    pub printings: HashMap<String, Vec<CardPrinting>>, // TODO(private)
}

impl CardData {
    pub fn from_bulk(bulk: HashMap<String, Vec<CardPrinting>>) -> Option<CardData> {
        let card_names = ScryfallCardNames::from_api_call()?;
        let lookup = CardNameLookup::from_card_names(&card_names.names);
        let printings: HashMap<String, Vec<CardPrinting>> = bulk
            .into_iter()
            .map(|(key, value)| (key.to_lowercase(), value))
            .collect();
        Some(CardData {
            card_names: card_names,
            lookup: lookup,
            printings: printings,
        })
    }

    pub fn update_names(&mut self) -> Option<()> {
        self.card_names = ScryfallCardNames::from_api_call()?;
        self.lookup = CardNameLookup::from_card_names(&self.card_names.names);
        Some(())
    }

    pub fn ensure_contains(&mut self, input_name: &str) -> Option<()> {
        let look = self.lookup.find(input_name);
        match look {
            Some(name) => {
                let entry = self.printings.entry(name);
                match entry {
                    Occupied(_) => {
                        debug!(
                            "there is card data for input name {} and standard name {}",
                            input_name,
                            entry.key()
                        );
                        Some(())
                    }
                    Vacant(token) => {
                        let scryfall_objects = query_scryfall_by_name(token.key());
                        match scryfall_objects {
                            Some(ref objects) => {
                                for object in objects.iter() {
                                    insert_scryfall_object(
                                        &mut self.printings,
                                        &self.card_names,
                                        object,
                                    );
                                }
                                Some(())
                            }
                            None => {
                                error!("querying scryfall for name {} failed", token.key());
                                None
                            }
                        }
                    }
                }
            }
            None => None,
        }
    }

    pub fn get_uris(&self, name: &str, set: Option<&str>) -> Option<(String, Option<String>)> {
        let standard_name = self.lookup.find(name)?;
        let p = self.printings.get(&standard_name)?;
        let matching = p
            .iter()
            .find(|printing| printing.set == set.unwrap_or(""))
            .unwrap_or(p.iter().next()?);
        Some((
            matching.border_crop.clone(),
            matching.border_crop_back.clone(),
        ))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct DecklistEntry {
    pub multiple: i32,
    pub card: Card,
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

impl<'a> ParsedDecklistLine<'a> {
    pub fn as_entry(&self) -> Option<DecklistEntry> {
        self.entry.clone()
    }
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

#[derive(Debug, PartialEq)]
pub enum BacksideMode {
    Zero,
    One,
    Matching,
    BackOnly,
}

impl<'v> FromFormValue<'v> for BacksideMode {
    type Error = &'v RawStr;

    fn from_form_value(form_value: &'v RawStr) -> Result<BacksideMode, &'v RawStr> {
        if form_value == "Zero" {
            Ok(BacksideMode::Zero)
        } else if form_value == "One" {
            Ok(BacksideMode::One)
        } else if form_value == "Matching" {
            Ok(BacksideMode::Matching)
        } else if form_value == "BackOnly" {
            Ok(BacksideMode::BackOnly)
        } else {
            Err(form_value)
        }
    }
}

pub fn encode_card_name(name: &str) -> String {
    name.replace(" ", "+")
}

pub fn query_image(card: &Card) -> Option<DynamicImage> {
    let mut uri = format!(
        "https://api.scryfall.com/cards/named?fuzzy={}&version=border_crop&format=image",
        encode_card_name(card.name.as_str())
    );
    if card.set.is_some() {
        uri += format!("&set={}", card.set.as_ref().unwrap()).as_str();
    }
    query_image_uri(uri.as_str())
}

pub fn query_image_uri(uri: &str) -> Option<DynamicImage> {
    debug!("scryfall uri: {}", uri);

    let request = scryfall_call(uri);
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

pub fn query_scryfall_object(
    name: &str,
    set: Option<&str>,
) -> Option<serde_json::Map<String, Value>> {
    let mut uri = format!(
        "https://api.scryfall.com/cards/named?exact={}&format=json",
        encode_card_name(name)
    );
    if set.is_some() {
        uri += format!("&set={}", set.as_ref().unwrap()).as_str();
    }
    let request = scryfall_call(&uri);
    match request {
        Ok(reqok) => serde_json::from_reader(reqok).ok(),
        Err(e) => {
            info!("error in scryfall object request: {}", e);
            return None;
        }
    }
}

pub fn query_scryfall_by_name(name: &str) -> Option<Vec<serde_json::Map<String, Value>>> {
    let uri = format!(
        "https://api.scryfall.com/cards/search?q=name=!{}&unique=prints",
        encode_card_name(name)
    );
    let request = scryfall_call(&uri);
    match request {
        Ok(reqok) => {
            let answer: ScryfallSearchAnswer = serde_json::from_reader(reqok).ok()?;
            Some(answer.data)
        }
        Err(e) => {
            info!("error in scryfall search request by name: {}", e);
            return None;
        }
    }
}

pub struct CachedImageResponse {
    t: DateTime<Utc>,
    image: DynamicImage,
}

impl CachedImageResponse {
    pub fn from_image(i: DynamicImage) -> CachedImageResponse {
        CachedImageResponse {
            t: Utc::now(),
            image: i,
        }
    }
}

impl fmt::Display for CachedImageResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "created at {}", self.t)
    }
}

impl fmt::Debug for CachedImageResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

pub struct ScryfallCache {
    last_purge: DateTime<Utc>,
    images: std::collections::HashMap<String, CachedImageResponse>,
}

impl ScryfallCache {
    fn get_max_age() -> chrono::Duration {
        chrono::Duration::days(14)
    }

    pub fn new() -> ScryfallCache {
        ScryfallCache {
            last_purge: Utc::now(),
            images: std::collections::HashMap::new(),
        }
    }

    pub fn ensure_contains(&mut self, uri: &str) -> Option<()> {
        let entry = self.images.entry(uri.to_string());
        match entry {
            Occupied(_) => {
                debug!("image uri cached: {}", uri);
                Some(())
            }
            Vacant(token) => {
                let image_query = query_image_uri(uri);
                match image_query {
                    Some(image) => {
                        token.insert(CachedImageResponse {
                            t: Utc::now(),
                            image: image,
                        });
                        Some(())
                    }
                    None => {
                        error!("calling image from uri {} failed", uri);
                        None
                    }
                }
            }
        }
    }

    pub fn get(&self, uri: &str) -> Option<&DynamicImage> {
        self.images.get(uri).map(|ci| &ci.image)
    }

    pub fn list(&self) -> String {
        let mut desc: String = "<ul>".to_string();
        for (key, value) in &self.images {
            desc.push_str(format!("<li>{}: {:?}</li>", key, value).as_str());
        }
        desc.push_str("</ul>");
        desc
    }

    pub fn purge(&mut self, max_age: Option<chrono::Duration>) {
        let n = Utc::now();
        debug!("{} cached responses before purging", self.images.len());
        self.images
            .retain(|_, value| n - value.t < max_age.unwrap_or(ScryfallCache::get_max_age()));
        self.last_purge = n;
        debug!("{} cached responses after purging", self.images.len());
    }
}

pub fn expand_multiples(entry: DecklistEntry) -> itertools::RepeatN<Card> {
    itertools::repeat_n(entry.card, usize::try_from(entry.multiple).unwrap_or(0))
}

pub fn images_to_page<'a, I>(mut it: I) -> Option<RgbaImage>
where
    I: Iterator<Item = &'a DynamicImage>,
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
                    im,
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

// pub fn decklist_to_pdf(cache: &mut ScryfallCache, decklist: &str) -> Option<Vec<u8>> {
//     pages_to_pdf(
//         parse_decklist(decklist)
//             .into_iter()
//             .flat_map(|parsed| parsed.entry.into_iter())
//             .flat_map(expand_multiples)
//             .map(|it| cache.query_image(&it))
//             .flat_map(|e| e.into_iter())
//             .batching(|it| images_to_page(it)),
//     )
// }

#[derive(Debug)]
pub struct CardNameLookup {
    corpus: Corpus,
    partial_to_full: HashMap<String, String>,
}

impl CardNameLookup {
    const THRESHOLD: f32 = 0.25;

    fn new() -> CardNameLookup {
        CardNameLookup {
            corpus: CorpusBuilder::new().finish(),
            partial_to_full: HashMap::new(),
        }
    }

    pub fn from_card_names(names: &Vec<String>) -> CardNameLookup {
        let mut lookup = CardNameLookup::new();
        for name in names.iter() {
            lookup.insert(name);
        }
        return lookup;
    }

    fn insert(&mut self, name_uppercase: &str) {
        let name = name_uppercase.to_lowercase();
        if name.contains("//") {
            for part in name.split("//") {
                self.corpus.add_text(part);
                self.partial_to_full
                    .insert(part.to_string(), name.to_string());
            }
        } else {
            self.corpus.add_text(&name);
        }
    }

    pub fn find(&self, name: &str) -> Option<String> {
        let best_match: String = self
            .corpus
            .search(name, CardNameLookup::THRESHOLD)
            .into_iter()
            .next()?
            .text;
        // let best_match = &matches.get(0)?.text;
        let full = self.partial_to_full.get(&best_match);
        match full {
            Some(_) => full.map(|f| f.clone()),
            None => Some(best_match),
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

    #[test]
    fn name_search() {
        let card_names: Vec<String> = vec![
            "Okaun, Eye of Chaos".to_string(),
            "Cut // Ribbons".to_string(),
        ];
        let lookup = CardNameLookup::from_card_names(&card_names);
        assert_eq!(
            lookup.find("okaun"),
            Some("okaun, eye of chaos".to_string())
        );
        assert_eq!(lookup.find("cut"), Some("cut // ribbons".to_string()));
        assert_eq!(lookup.find("ribbon"), Some("cut // ribbons".to_string()));
    }
}
