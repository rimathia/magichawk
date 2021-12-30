extern crate chrono;
#[macro_use]
extern crate lazy_static;
extern crate log;
extern crate printpdf;
extern crate regex;
extern crate reqwest;
extern crate rocket;
extern crate serde;
extern crate serde_json;
extern crate tokio;

use chrono::{DateTime, Utc};
use log::{debug, error, info};
use printpdf::image_crate::{imageops::overlay, DynamicImage, Rgb, RgbImage};
use regex::Match;
use regex::Regex;
use rocket::form::FromFormField;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::fmt;
use std::string::String;
use Option::{None, Some};

mod lookup;
use crate::lookup::{CardNameLookup, NameLookupResult, NameMatchMode};

mod pdf;
pub use crate::pdf::page_images_to_pdf;

mod scryfall;
pub use scryfall::{insert_scryfall_object, CardPrinting, Printings, ScryfallCardNames};
use scryfall::{query_scryfall_by_name, ScryfallCard};

mod scryfall_client;
pub use crate::scryfall_client::ScryfallClient;

pub const IMAGE_WIDTH: u32 = 480;
pub const IMAGE_HEIGHT: u32 = 680;

pub const PAGE_WIDTH: u32 = 3 * IMAGE_WIDTH;
pub const PAGE_HEIGHT: u32 = 3 * IMAGE_HEIGHT;

pub const IMAGE_HEIGHT_CM: f64 = 8.7;
pub const IMAGE_WIDTH_CM: f64 = IMAGE_HEIGHT_CM * IMAGE_WIDTH as f64 / IMAGE_HEIGHT as f64;

pub struct ImageLine {
    pub card: ScryfallCard,
    pub front: i32,
    pub back: i32,
}

pub struct CardData {
    pub card_names: ScryfallCardNames,
    pub lookup: CardNameLookup,
    pub printings: Printings,
}

impl CardData {
    pub async fn from_bulk(bulk: Printings, client: &ScryfallClient) -> Option<CardData> {
        let card_names = ScryfallCardNames::from_api_call(client).await?;
        let lookup = CardNameLookup::from_card_names(&card_names.names);
        let printings: Printings = bulk
            .into_iter()
            .map(|(key, value)| (key.to_lowercase(), value))
            .collect();
        Some(CardData {
            card_names,
            lookup,
            printings,
        })
    }

    pub async fn update_names(&mut self, client: &ScryfallClient) -> Option<()> {
        self.card_names = ScryfallCardNames::from_api_call(client).await?;
        self.lookup = CardNameLookup::from_card_names(&self.card_names.names);
        Some(())
    }

    async fn ensure_contains(&mut self, lookup: &NameLookupResult, client: &ScryfallClient) {
        let entry = self.printings.entry(lookup.name.clone());
        match entry {
            Occupied(_) => {
                debug!("there is card data for name {}", entry.key());
            }
            Vacant(token) => {
                let scryfall_objects = query_scryfall_by_name(token.key(), client).await;
                match scryfall_objects {
                    Some(ref objects) => {
                        for object in objects.iter() {
                            insert_scryfall_object(&mut self.printings, &self.card_names, object);
                        }
                    }
                    None => {
                        error!("querying scryfall for name {} failed", token.key());
                    }
                }
            }
        }
    }

    pub async fn get_card(
        &mut self,
        entry: &DecklistEntry,
        default_mode: BacksideMode,
        client: &ScryfallClient,
    ) -> Option<ImageLine> {
        let namelookup = self.lookup.find(&entry.name)?;
        debug!("namelookup in get_card: {:?}", namelookup);
        let backside = match namelookup.hit {
            NameMatchMode::Part(1) => BacksideMode::BackOnly,
            _ => default_mode,
        };
        debug!("backside in get_card: {:?}", backside);
        self.ensure_contains(&namelookup, client).await;
        let matchingprintings = self.printings.get(&namelookup.name)?;
        let printing = matchingprintings
            .iter()
            .find(|p| match &entry.set {
                Some(s) => p.set == s.to_lowercase(),
                None => false,
            })
            .unwrap_or(matchingprintings.iter().next()?);
        let frontmult = if backside == BacksideMode::BackOnly && printing.border_crop_back.is_some()
        {
            0
        } else {
            entry.multiple
        };
        let backmult = if printing.border_crop_back.is_some() {
            match backside {
                BacksideMode::Zero => 0,
                BacksideMode::One => 1,
                BacksideMode::Matching | BacksideMode::BackOnly => entry.multiple,
            }
        } else {
            0
        };
        debug!("frontmult: {}, backmult: {}", frontmult, backmult);
        Some(ImageLine {
            front: frontmult,
            back: backmult,
            card: ScryfallCard {
                name: namelookup.name,
                printing: printing.clone(),
            },
        })
    }
}

pub async fn image_lines_from_decklist(
    parsed: Vec<ParsedDecklistLine<'_>>,
    card_data: &mut CardData,
    default_backside_mode: BacksideMode,
    client: &ScryfallClient,
) -> Vec<ImageLine> {
    let mut image_lines = Vec::<ImageLine>::new();
    for line in parsed {
        let entry = &line.as_entry();
        match entry {
            Some(entry) => {
                if let Some(image_line) = card_data
                    .get_card(entry, default_backside_mode, client)
                    .await
                {
                    image_lines.push(image_line);
                }
            }
            None => {}
        }
    }
    image_lines
}

#[derive(Debug, PartialEq, Clone)]
pub struct DecklistEntry {
    pub multiple: i32,
    pub name: String,
    pub set: Option<String>,
}

impl DecklistEntry {
    pub fn new(m: i32, n: &str, s: Option<&str>) -> DecklistEntry {
        DecklistEntry {
            multiple: m,
            name: n.to_string(),
            set: s.map(|x| x.to_string()),
        }
    }

    pub fn from_name(n: &str) -> DecklistEntry {
        DecklistEntry {
            multiple: 1,
            name: n.to_string(),
            set: None,
        }
    }

    pub fn from_multiple_name(m: i32, n: &str) -> DecklistEntry {
        DecklistEntry {
            multiple: m,
            name: n.to_string(),
            set: None,
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
            Regex::new(r"^\s*(\d*)\s*([^\(\[\$\t]*)[\s\(\[]*([\dA-Za-z]{3})?").unwrap();
    }

    match REMNS.captures(line) {
        Some(mns) => {
            let multiple = parse_multiple(mns.get(1));
            let name = mns.get(2)?.as_str().trim().to_string();
            let set = parse_set(mns.get(3)).map(|s| s.to_string());
            let name_lowercase = name.to_lowercase();
            let non_entries = vec!["deck", "decklist", "sideboard"];
            if non_entries.iter().any(|s| **s == name_lowercase) {
                None
            } else {
                Some(DecklistEntry {
                    multiple,
                    name,
                    set,
                })
            }
        }
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

#[derive(Debug, PartialEq, Copy, Clone, FromFormField)]
pub enum BacksideMode {
    Zero,
    One,
    Matching,
    BackOnly,
}

pub async fn query_image_uri(uri: &str, client: &ScryfallClient) -> Option<DynamicImage> {
    debug!("scryfall uri: {}", uri);

    let request = client.call(uri).await;
    match request {
        Ok(response) => match response.bytes().await {
            Ok(b) => match image::load_from_memory_with_format(&b, image::ImageFormat::Jpeg) {
                Ok(im) => Some(im),
                Err(e) => {
                    error!("error converting response to jpeg: {}", e);
                    None
                }
            },
            Err(e) => {
                info!("error in getting bytes of image: {}", e);
                None
            }
        },
        Err(e) => {
            info!("error in image request: {}", e);
            None
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

    pub async fn ensure_contains(&mut self, uri: &str, client: &ScryfallClient) -> Option<()> {
        let entry = self.images.entry(uri.to_string());
        match entry {
            Occupied(_) => {
                debug!("image uri cached: {}", uri);
                Some(())
            }
            Vacant(token) => {
                let image_query = query_image_uri(uri, client).await;
                match image_query {
                    Some(image) => {
                        token.insert(CachedImageResponse {
                            t: Utc::now(),
                            image,
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

    pub async fn ensure_contains_line(&mut self, line: &ImageLine, client: &ScryfallClient) {
        if line.front > 0 {
            self.ensure_contains(&line.card.printing.border_crop, client)
                .await;
        }
        if line.back > 0 {
            match &line.card.printing.border_crop_back {
                Some(uri) => {
                    self.ensure_contains(uri, client).await;
                }
                None => {}
            }
        }
    }

    pub fn get(&self, uri: &str) -> Option<&DynamicImage> {
        self.images.get(uri).map(|ci| &ci.image)
    }

    pub fn list(&self) -> String {
        let mut desc: String = format!("last purged at {}", self.last_purge);
        desc += "<table>\n<tbody>";
        for (key, value) in &self.images {
            desc.push_str(format!("<tr><td>{}</td><td>{:?}</td></tr>\n", key, value).as_str());
        }
        desc += "\n</tbody>\n</table>";
        desc
    }

    pub fn purge(&mut self, max_age: Option<chrono::Duration>) {
        let n = Utc::now();
        debug!("{} cached responses before purging", self.images.len());
        self.images
            .retain(|_, value| n - value.t < max_age.unwrap_or_else(ScryfallCache::get_max_age));
        self.last_purge = n;
        debug!("{} cached responses after purging", self.images.len());
    }
}

impl Default for ScryfallCache {
    fn default() -> Self {
        Self::new()
    }
}

pub fn images_to_page<'a, I>(mut it: I) -> Option<DynamicImage>
where
    I: Iterator<Item = &'a DynamicImage>,
{
    let mut pos_hor = 0;
    let mut pos_ver = 0;

    let mut composed: Option<RgbImage> = None;
    let white_pixel = Rgb::<u8>([255, 255, 255]);

    loop {
        match it.next() {
            None => return composed.map(DynamicImage::ImageRgb8),
            Some(im) => {
                let without_alpha: RgbImage = im.to_rgb8();
                overlay(
                    composed.get_or_insert(RgbImage::from_pixel(
                        PAGE_WIDTH,
                        PAGE_HEIGHT,
                        white_pixel,
                    )),
                    &without_alpha,
                    pos_hor * IMAGE_WIDTH,
                    pos_ver * IMAGE_HEIGHT,
                );
                pos_hor += 1;
                if pos_hor == 3 {
                    pos_hor = 0;
                    pos_ver += 1;
                }
                if pos_ver == 3 {
                    return composed.map(DynamicImage::ImageRgb8);
                }
            }
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
    fn shatter() {
        assert_eq!(
            parse_line("1 shatter [mrd]").unwrap(),
            DecklistEntry::new(1, "shatter", Some("mrd"))
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
                entry: None,
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
    fn arenaexport2() {
        let decklist = "Deck\n1 Defiant Strike (M21) 15\n24 Plains (ANB) 115\n\nSideboard\n2 Faerie Guidemother (ELD) 11";
        let expected = vec![
            ParsedDecklistLine {
                line: "Deck",
                entry: None,
            },
            ParsedDecklistLine {
                line: "1 Defiant Strike (M21) 15",
                entry: Some(DecklistEntry::new(1, "Defiant Strike", Some("M21"))),
            },
            ParsedDecklistLine {
                line: "24 Plains (ANB) 115",
                entry: Some(DecklistEntry::new(24, "Plains", Some("ANB"))),
            },
            ParsedDecklistLine {
                line: "Sideboard",
                entry: None,
            },
            ParsedDecklistLine {
                line: "2 Faerie Guidemother (ELD) 11",
                entry: Some(DecklistEntry::new(2, "Faerie Guidemother", Some("ELD"))),
            },
        ];
        let parsed = parse_decklist(decklist);
        for (left, right) in parsed.iter().zip(expected.iter()) {
            assert_eq!(left, right);
        }

        // not necessary anymore because we filter out the lines "deck" and "sideboard" manually now
        // let mut card_data = CardData::from_bulk(
        //     serde_json::from_reader(
        //         //serde_json::from_reader::<HashMap<String, Vec<CardPrinting>>(
        //         std::fs::File::open("assets/card_data.json").unwrap(),
        //     )
        //     .unwrap(),
        // )
        // .unwrap();

        // let imagelines = image_lines_from_decklist(parsed, &mut card_data, BacksideMode::One);
        // assert_eq!(imagelines.len(), 3);
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
            Some(NameLookupResult {
                name: "okaun, eye of chaos".to_string(),
                hit: NameMatchMode::Full
            })
        );
        assert_eq!(
            lookup.find("cut // ribbon"),
            Some(NameLookupResult {
                name: "cut // ribbons".to_string(),
                hit: NameMatchMode::Full
            })
        );
        assert_eq!(
            lookup.find("cut"),
            Some(NameLookupResult {
                name: "cut // ribbons".to_string(),
                hit: NameMatchMode::Part(0)
            })
        );
        assert_eq!(
            lookup.find("ribbon"),
            Some(NameLookupResult {
                name: "cut // ribbons".to_string(),
                hit: NameMatchMode::Part(1)
            })
        );
    }
}
