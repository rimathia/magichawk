extern crate chrono;
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
use rocket::form::FromFormField;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::fmt;
use std::string::String;
use Option::{None, Some};

mod decklist;
pub use crate::decklist::parse_decklist;
use crate::decklist::{DecklistEntry, ParsedDecklistLine};

mod lookup;
use crate::lookup::{CardNameLookup, NameLookupResult, NameMatchMode};

mod pdf;
pub use crate::pdf::page_images_to_pdf;

mod scryfall;
use scryfall::query_scryfall_by_name;
pub use scryfall::{
    insert_scryfall_object, CardPrintings, MinimalScryfallObject, ScryfallCardNames,
};

mod scryfall_client;
pub use crate::scryfall_client::ScryfallClient;

pub const IMAGE_WIDTH: u32 = 480;
pub const IMAGE_HEIGHT: u32 = 680;

pub const PAGE_WIDTH: u32 = 3 * IMAGE_WIDTH;
pub const PAGE_HEIGHT: u32 = 3 * IMAGE_HEIGHT;

pub const IMAGE_HEIGHT_CM: f64 = 8.7;
pub const IMAGE_WIDTH_CM: f64 = IMAGE_HEIGHT_CM * IMAGE_WIDTH as f64 / IMAGE_HEIGHT as f64;

pub struct ImageLine {
    pub name: String,
    pub images: Vec<(String, i32)>,
}

pub struct CardData {
    pub card_names: ScryfallCardNames,
    pub lookup: CardNameLookup,
    pub printings: CardPrintings,
}

impl CardData {
    pub async fn from_bulk(bulk: CardPrintings, client: &ScryfallClient) -> Option<CardData> {
        let card_names = ScryfallCardNames::from_api_call(client).await?;
        let lookup = CardNameLookup::from_card_names(&card_names.names);
        let printings: CardPrintings = bulk
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
        let set_matches = |p: &&MinimalScryfallObject| match &entry.set {
            Some(s) => p.set == s.to_lowercase(),
            None => false,
        };
        let mut printing = matchingprintings
            .iter()
            .find(set_matches)
            .unwrap_or(matchingprintings.iter().next()?)
            .clone();
        match &printing.meld_result {
            Some(meld_result) => {
                let meld_result_lookup = self.lookup.find(&meld_result);
                match meld_result_lookup {
                    Some(meld_result_lookup) => {
                        self.ensure_contains(&meld_result_lookup, client).await;
                        let matchingprintings_meld =
                            self.printings.get(&meld_result_lookup.name)?;
                        let printing_meld = matchingprintings_meld
                            .iter()
                            .find(set_matches)
                            .unwrap_or(matchingprintings_meld.iter().next()?);
                        printing.border_crop_back = Some(printing_meld.border_crop.clone());
                    }
                    None => {}
                }
            }
            None => {}
        }
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
        let mut images = Vec::new();
        if frontmult > 0 {
            images.push((printing.border_crop, frontmult))
        }
        if backmult > 0 {
            images.push((printing.border_crop_back.unwrap(), backmult))
        }
        Some(ImageLine {
            name: entry.name.clone(),
            images: images,
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

#[derive(Debug, PartialEq, Eq, Copy, Clone, FromFormField)]
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
        for (uri, _mult) in &line.images {
            self.ensure_contains(&uri, client).await;
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
                    (pos_hor * IMAGE_WIDTH).into(),
                    (pos_ver * IMAGE_HEIGHT).into(),
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
