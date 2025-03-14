extern crate log;
extern crate printpdf;
extern crate regex;
extern crate reqwest;
extern crate rocket;
extern crate serde;
extern crate serde_json;
extern crate time;
extern crate tokio;

use Option::{None, Some};
use log::{debug, error, info};
use printpdf::image_crate::{
    DynamicImage, ImageFormat, Rgb, RgbImage, imageops::overlay, load_from_memory_with_format,
};
use rocket::form::FromFormField;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::fmt;
use std::string::String;
use time::OffsetDateTime;

mod decklist;
pub use crate::decklist::parse_decklist;
use crate::decklist::{DecklistEntry, ParsedDecklistLine};

mod lookup;
use crate::lookup::{CardNameLookup, NameLookupResult, NameMatchMode};

mod pdf;
pub use crate::pdf::page_images_to_pdf;

mod scryfall;
pub use scryfall::{
    CardPrintings, MinimalScryfallObject, ScryfallCardNames, get_minimal_card_printings,
    insert_scryfall_object,
};
use scryfall::{get_minimal_scryfall_languages, query_scryfall_by_name};

mod scryfall_client;
pub use crate::scryfall_client::ScryfallClient;
pub use crate::scryfall_client::blocking_call;

pub const IMAGE_WIDTH: u32 = 480;
pub const IMAGE_HEIGHT: u32 = 680;

pub const PAGE_WIDTH: u32 = 3 * IMAGE_WIDTH;
pub const PAGE_HEIGHT: u32 = 3 * IMAGE_HEIGHT;

pub const IMAGE_HEIGHT_CM: f32 = 8.7;
pub const IMAGE_WIDTH_CM: f32 = IMAGE_HEIGHT_CM * IMAGE_WIDTH as f32 / IMAGE_HEIGHT as f32;

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
    pub async fn from_client(client: &ScryfallClient) -> Option<CardData> {
        let card_names = ScryfallCardNames::from_api_call(client).await?;

        let lookup = CardNameLookup::from_card_names(&card_names.names);
        Some(CardData {
            card_names,
            lookup,
            printings: CardPrintings::new(),
        })
    }

    pub async fn from_printings(
        printings: CardPrintings,
        client: &ScryfallClient,
    ) -> Option<CardData> {
        let card_names = ScryfallCardNames::from_api_call(client).await?;
        let lookup = CardNameLookup::from_card_names(&card_names.names);
        let printings_lowercase = printings
            .printings
            .into_iter()
            .map(|(key, value)| (key.to_lowercase(), value))
            .collect();
        let mut languages = get_minimal_scryfall_languages();
        languages.extend(printings.languages);
        Some(CardData {
            card_names,
            lookup,
            printings: CardPrintings {
                printings: printings_lowercase,
                languages,
            },
        })
    }

    pub async fn update_names(&mut self, client: &ScryfallClient) -> Option<()> {
        self.card_names = ScryfallCardNames::from_api_call(client).await?;
        self.lookup = CardNameLookup::from_card_names(&self.card_names.names);
        Some(())
    }

    async fn ensure_contains(&mut self, lookup: &NameLookupResult, client: &ScryfallClient) {
        let entry = self.printings.printings.entry(lookup.name.clone());
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
        let matchingprintings = self.printings.printings.get(&namelookup.name)?;
        let set_matches = |p: &&MinimalScryfallObject| match &entry.set {
            Some(s) => p.set == s.to_lowercase(),
            None => false,
        };
        let lang_matches = |p: &&MinimalScryfallObject| match &entry.lang {
            Some(lang) => p.language == lang.to_lowercase(),
            None => false,
        };
        let printing_right_set = matchingprintings.iter().find(set_matches);
        let printing_right_lang = matchingprintings.iter().find(lang_matches);
        let mut printing = if printing_right_set.is_some() {
            printing_right_set?.clone()
        } else if printing_right_lang.is_some() {
            printing_right_lang?.clone()
        } else {
            let lang_en = |p: &&MinimalScryfallObject| p.language.to_lowercase() == "en";
            let printing_en = matchingprintings.iter().find(lang_en);
            printing_en
                .unwrap_or(matchingprintings.iter().next()?)
                .clone()
        };
        if let Some(meld_result) = &printing.meld_result {
            let meld_result_lookup = self.lookup.find(meld_result);
            if let Some(meld_result_lookup) = meld_result_lookup {
                self.ensure_contains(&meld_result_lookup, client).await;
                let matchingprintings_meld =
                    self.printings.printings.get(&meld_result_lookup.name)?;
                let printing_meld = matchingprintings_meld
                    .iter()
                    .find(set_matches)
                    .unwrap_or(matchingprintings_meld.iter().next()?);
                printing.border_crop_back = Some(printing_meld.border_crop.clone());
            }
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
            images,
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
        if let Some(entry) = entry {
            if let Some(image_line) = card_data
                .get_card(entry, default_backside_mode, client)
                .await
            {
                image_lines.push(image_line);
            }
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
            Ok(b) => match load_from_memory_with_format(&b, ImageFormat::Jpeg) {
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
    t: OffsetDateTime,
    image: DynamicImage,
}

impl CachedImageResponse {
    pub fn from_image(i: DynamicImage) -> CachedImageResponse {
        CachedImageResponse {
            t: OffsetDateTime::now_utc(),
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
    last_purge: OffsetDateTime,
    images: std::collections::HashMap<String, CachedImageResponse>,
}

impl ScryfallCache {
    fn get_max_age() -> time::Duration {
        time::Duration::days(14)
    }

    pub fn new() -> ScryfallCache {
        ScryfallCache {
            last_purge: OffsetDateTime::now_utc(),
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
                            t: OffsetDateTime::now_utc(),
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
            self.ensure_contains(uri, client).await;
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

    pub fn purge(&mut self, max_age: Option<time::Duration>) {
        let n = OffsetDateTime::now_utc();
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
