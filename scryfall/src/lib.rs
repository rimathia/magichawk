//use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Color {
    #[serde(rename = "W")]
    White,

    #[serde(rename = "U")]
    Blue,

    #[serde(rename = "B")]
    Black,

    #[serde(rename = "R")]
    Red,

    #[serde(rename = "G")]
    Green,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum ProducedMana {
    #[serde(rename = "W")]
    White,

    #[serde(rename = "U")]
    Blue,

    #[serde(rename = "B")]
    Black,

    #[serde(rename = "R")]
    Red,

    #[serde(rename = "G")]
    Green,

    #[serde(rename = "C")]
    Colorless,

    #[serde(rename = "S")]
    Snow,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Language {
    #[serde(rename = "en")]
    En,
    #[serde(rename = "es")]
    Es,
    #[serde(rename = "fr")]
    Fr,
    #[serde(rename = "de")]
    De,
    #[serde(rename = "it")]
    It,
    #[serde(rename = "pt")]
    Pt,
    #[serde(rename = "ja")]
    Ja,
    #[serde(rename = "ko")]
    Ko,
    #[serde(rename = "ru")]
    Ru,
    #[serde(rename = "zhs")]
    Zhs,
    #[serde(rename = "zht")]
    Zht,
    #[serde(rename = "he")]
    He,
    #[serde(rename = "la")]
    La,
    #[serde(rename = "grc")]
    Grc,
    #[serde(rename = "ar")]
    Ar,
    #[serde(rename = "sa")]
    Sa,
    #[serde(rename = "ph")]
    Ph,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum FrameType {
    #[serde(rename = "1993")]
    Year1993,

    #[serde(rename = "1997")]
    Year1997,

    #[serde(rename = "2003")]
    Year2003,

    #[serde(rename = "2015")]
    Year2015,

    #[serde(rename = "future")]
    Future,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum FrameLayout {
    #[serde(rename = "normal")]
    Normal,

    #[serde(rename = "flip")]
    Flip,

    #[serde(rename = "split")]
    Split,

    #[serde(rename = "transform")]
    Transform,

    #[serde(rename = "modal_dfc")]
    ModalDfc,

    #[serde(rename = "meld")]
    Meld,

    #[serde(rename = "leveler")]
    Leveler,

    #[serde(rename = "class")]
    Class,

    #[serde(rename = "saga")]
    Saga,

    #[serde(rename = "adventure")]
    Adventure,

    #[serde(rename = "planar")]
    Planar,

    #[serde(rename = "scheme")]
    Scheme,

    #[serde(rename = "vanguard")]
    Vanguard,

    #[serde(rename = "token")]
    Token,

    #[serde(rename = "double_faced_token")]
    DoubleFacedToken,

    #[serde(rename = "emblem")]
    Emblem,

    #[serde(rename = "augment")]
    Augment,

    #[serde(rename = "host")]
    Host,

    #[serde(rename = "art_series")]
    ArtSeries,

    #[serde(rename = "reversible_card")]
    ReversibleCard,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum ImageStatus {
    #[serde(rename = "missing")]
    Missing,

    #[serde(rename = "placeholder")]
    Placeholder,

    #[serde(rename = "lowres")]
    Lowres,

    #[serde(rename = "highres_scan")]
    HighresScan,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Object {
    #[serde(rename = "card")]
    Card,

    #[serde(rename = "related_card")]
    RelatedCard,

    #[serde(rename = "card_face")]
    CardFace,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Component {
    #[serde(rename = "token")]
    Token,

    #[serde(rename = "meld_part")]
    MeldPart,

    #[serde(rename = "meld_result")]
    MeldResult,

    #[serde(rename = "combo_piece")]
    CompoPiece,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Rarity {
    #[serde(rename = "common")]
    Common,

    #[serde(rename = "uncommon")]
    Uncommon,

    #[serde(rename = "rare")]
    Rare,

    #[serde(rename = "special")]
    Special,

    #[serde(rename = "mythic")]
    Mythic,

    #[serde(rename = "bonus")]
    Bonus,
}

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Debug)]
pub enum ImageFormat {
    #[serde(rename = "png")]
    Png,

    #[serde(rename = "border_crop")]
    BorderCrop,

    #[serde(rename = "art_crop")]
    ArtCrop,

    #[serde(rename = "large")]
    Large,

    #[serde(rename = "normal")]
    Normal,

    #[serde(rename = "small")]
    Small,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Legality {
    #[serde(rename = "not_legal")]
    NotLegal,

    #[serde(rename = "legal")]
    Legal,

    #[serde(rename = "restricted")]
    Restricted,

    #[serde(rename = "banned")]
    Banned,
}

#[derive(Serialize, Deserialize, Debug, Hash, PartialEq, Eq)]
pub enum Format {
    #[serde(rename = "standard")]
    Standard,

    #[serde(rename = "future")]
    Future,

    #[serde(rename = "historic")]
    Historic,

    #[serde(rename = "pioneer")]
    Pioneer,

    #[serde(rename = "modern")]
    Modern,

    #[serde(rename = "legacy")]
    Legacy,

    #[serde(rename = "pauper")]
    Pauper,

    #[serde(rename = "vintage")]
    Vintage,

    #[serde(rename = "penny")]
    Penny,

    #[serde(rename = "commander")]
    Commander,

    #[serde(rename = "brawl")]
    Brawl,

    #[serde(rename = "duel")]
    Duel,

    #[serde(rename = "oldschool")]
    Oldschool,

    #[serde(rename = "gladiator")]
    Gladiator,

    #[serde(rename = "historicbrawl")]
    HistoricBrawl,

    #[serde(rename = "alchemy")]
    Alchemy,

    #[serde(rename = "paupercommander")]
    PauperCommander,

    #[serde(rename = "premodern")]
    Premodern,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Preview {
    pub previewed_at: String,
    pub source_uri: String,
    pub source: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum SecurityStamp {
    #[serde(rename = "oval")]
    Oval,

    #[serde(rename = "triangle")]
    Triangle,

    #[serde(rename = "acorn")]
    Acorn,

    #[serde(rename = "arena")]
    Arena,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Game {
    #[serde(rename = "paper")]
    Paper,

    #[serde(rename = "arena")]
    Arena,

    #[serde(rename = "mtgo")]
    Mtgo,

    #[serde(rename = "astral")]
    Astral,

    #[serde(rename = "sega")]
    Sega,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum BorderColor {
    #[serde(rename = "black")]
    Black,

    #[serde(rename = "white")]
    White,

    #[serde(rename = "borderless")]
    Borderless,

    #[serde(rename = "silver")]
    Silver,

    #[serde(rename = "gold")]
    Gold,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Finish {
    #[serde(rename = "nonfoil")]
    NonFoil,

    #[serde(rename = "foil")]
    Foil,

    #[serde(rename = "etched")]
    Etched,

    #[serde(rename = "glossy")]
    Glossy,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum FrameEffect {
    #[serde(rename = "sunmoondfc")]
    SunMoonDfc,

    #[serde(rename = "mooneldrazidfc")]
    MoonEldraziDfc,

    #[serde(rename = "compasslanddfc")]
    CompassLandDfc,

    #[serde(rename = "originpwdfc")]
    OriginPwDfc,

    #[serde(rename = "waxingandwaningmoondfc")]
    WaxingAndWaningMoonDfc,

    #[serde(rename = "legendary")]
    Legendary,

    #[serde(rename = "nyxtouched")]
    NyxTouched,

    #[serde(rename = "devoid")]
    Devoid,

    #[serde(rename = "tombstone")]
    Tombstone,

    #[serde(rename = "snow")]
    Snow,

    #[serde(rename = "lesson")]
    Lesson,

    #[serde(rename = "draft")]
    Draft,

    #[serde(rename = "inverted")]
    Inverted,

    #[serde(rename = "colorshifted")]
    Colorshifted,

    #[serde(rename = "miracle")]
    Miracle,

    #[serde(rename = "companion")]
    Companion,

    #[serde(rename = "extendedart")]
    ExtendedArt,

    #[serde(rename = "booster")]
    Booster,

    #[serde(rename = "showcase")]
    Showcase,

    #[serde(rename = "fullart")]
    Fullart,

    #[serde(rename = "etched")]
    Etched,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CardFace {
    pub artist: Option<String>,
    pub cmc: Option<f64>,
    pub color_indicator: Option<Vec<Color>>,
    pub colors: Option<Vec<Color>>,
    pub flavor_text: Option<String>,
    pub illustration_id: Option<Uuid>,
    pub image_uris: Option<HashMap<ImageFormat, String>>,
    pub layout: Option<FrameLayout>,
    pub loyalty: Option<String>,
    pub mana_cost: Option<String>,
    pub name: String,
    pub object: Object,
    pub oracle_id: Option<Uuid>,
    pub oracle_text: Option<String>,
    pub power: Option<String>,
    pub printed_name: Option<String>,
    pub printed_text: Option<String>,
    pub printed_type_line: Option<String>,
    pub toughness: Option<String>,
    pub type_line: Option<String>,
    pub watermark: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RelatedCard {
    pub id: Uuid,
    pub object: Object,
    pub component: Component,
    pub name: String,
    pub type_line: String,
    pub uri: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Card {
    pub arena_id: Option<u32>,
    pub id: Uuid,
    pub lang: Language,
    pub mtgo_id: Option<u32>,
    pub mtgo_foil_id: Option<u32>,
    pub multiverse_ids: Vec<u32>,
    pub tcgplayer_id: Option<u32>,
    pub tcgplayer_etched_id: Option<u32>,
    pub cardmarket_id: Option<u32>,
    pub object: Object,
    pub oracle_id: Option<Uuid>,
    pub prints_search_uri: String,
    pub rulings_uri: String,
    pub scryfall_uri: String,
    pub uri: String,

    pub all_parts: Option<Vec<RelatedCard>>,
    pub card_faces: Option<Vec<CardFace>>,
    pub cmc: Option<f64>,
    pub color_identity: Vec<Color>,
    pub color_indicator: Option<Vec<Color>>,
    pub colors: Option<Vec<Color>>,
    pub edhrec_rank: Option<u32>,
    pub hand_modifier: Option<String>,
    pub keywords: Vec<String>,
    pub layout: FrameLayout,
    pub legalities: HashMap<Format, Legality>,
    pub life_modifier: Option<String>,
    pub loyalty: Option<String>,
    pub mana_cost: Option<String>,
    pub name: String,
    pub oracle_text: Option<String>,
    pub oversized: bool,
    pub power: Option<String>,
    pub produced_mana: Option<Vec<ProducedMana>>,
    pub reserved: bool,
    pub toughness: Option<String>,
    pub type_line: Option<String>,

    pub artist: Option<String>,
    pub booster: bool,
    pub border_color: BorderColor,
    pub card_back_id: Option<Uuid>,
    pub collector_number: String,
    pub content_warning: Option<bool>,
    pub digital: bool,
    pub finishes: Vec<Finish>,
    pub flavor_name: Option<String>,
    pub flavor_text: Option<String>,
    pub frame_effects: Option<Vec<FrameEffect>>,
    pub frame: FrameType,
    pub full_art: bool,
    pub games: Vec<Game>,
    pub highres_image: bool,
    pub illustration_id: Option<Uuid>,
    pub image_status: Option<ImageStatus>,
    pub image_uris: Option<HashMap<ImageFormat, String>>,
    pub prices: HashMap<String, Option<String>>,
    pub printed_name: Option<String>,
    pub printed_text: Option<String>,
    pub printed_type_line: Option<String>,
    pub promo: bool,
    pub promo_types: Option<Vec<String>>,
    pub purchase_uris: Option<HashMap<String, String>>,
    pub rarity: Rarity,
    pub related_uris: HashMap<String, String>,
    pub released_at: String,
    pub reprint: bool,
    pub scryfall_set_uri: String,
    pub set_name: String,
    pub set_search_uri: String,
    pub set_type: String,
    pub set_uri: String,
    pub set: String,
    pub set_id: Option<Uuid>,
    pub story_spotlight: bool,
    pub textless: bool,
    pub variation: bool,
    pub variation_of: Option<Uuid>,
    pub security_stamp: Option<SecurityStamp>,
    pub watermark: Option<String>,
    pub preview: Option<Preview>,
}

#[cfg(test)]
mod tests {
    use crate::Card;
    use std::fs::File;
    use std::path::Path;

    #[test]
    fn default_cards() {
        let f = File::open(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("test_input/oracle-cards-20211228_truncated.json"),
        )
        .expect("couldn't open test input file");

        let cards: Vec<Card> = serde_json::from_reader(f).unwrap();

        assert_eq!(cards.len(), 4, "check we get the expected number of cards");

        println!("{:#?}", cards);
    }
}
