use chrono::{DateTime, Utc};
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::scryfall_client::{blocking_call, ScryfallClient};

const SCRYFALL_CARD_NAMES: &str = "https://api.scryfall.com/catalog/card-names";

fn encode_card_name(name: &str) -> String {
    name.replace(' ', "+").replace("//", "")
}

#[derive(Serialize, Deserialize)]
pub struct ScryfallCardNames {
    pub object: String,
    pub uri: String,
    pub total_values: i32,
    pub date: Option<DateTime<Utc>>,
    #[serde(alias = "data")]
    pub names: Vec<String>,
}

impl ScryfallCardNames {
    pub async fn from_api_call(client: &ScryfallClient) -> Option<ScryfallCardNames> {
        let mut card_names: ScryfallCardNames = client
            .call(SCRYFALL_CARD_NAMES)
            .await
            .ok()?
            .json::<ScryfallCardNames>()
            .await
            .ok()?;
        card_names.date = Some(Utc::now());
        for name in card_names.names.iter_mut() {
            *name = name.to_lowercase();
        }
        Some(card_names)
    }

    pub fn from_api_call_blocking() -> Option<ScryfallCardNames> {
        let mut card_names = blocking_call(SCRYFALL_CARD_NAMES)
            .ok()?
            .json::<ScryfallCardNames>()
            .ok()?;
        card_names.date = Some(Utc::now());
        for name in card_names.names.iter_mut() {
            *name = name.to_lowercase();
        }
        Some(card_names)
    }
}

#[derive(Serialize, Deserialize)]
pub struct ScryfallSearchAnswer {
    pub object: String,
    pub total_cards: i32,
    pub has_more: bool,
    pub next_page: Option<String>,
    pub data: Vec<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CardPrinting {
    pub set: String,
    pub border_crop: String,
    pub border_crop_back: Option<String>,
}

pub type Printings = HashMap<String, Vec<CardPrinting>>;

#[derive(Serialize, Deserialize, Debug)]
pub struct ScryfallCard {
    pub name: String,
    pub printing: CardPrinting,
}

impl ScryfallCard {
    pub fn from_scryfall_object(
        d: &serde_json::Map<String, serde_json::Value>,
    ) -> Option<ScryfallCard> {
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
        Some(ScryfallCard {
            name: n,
            printing: CardPrinting {
                set: s,
                border_crop: bc,
                border_crop_back: bcb,
            },
        })
    }
}

pub fn insert_scryfall_card(
    printings: &mut Printings,
    card_names: &ScryfallCardNames,
    card: ScryfallCard,
) {
    let lowercase_name = card.name.to_lowercase();
    if card_names.names.contains(&lowercase_name) {
        printings
            .entry(lowercase_name)
            .or_insert_with(Vec::new)
            .push(card.printing);
    } else {
        error!(
            "couldn't insert scryfall card because name was unknown: {:?}",
            card
        )
    }
}

pub fn insert_scryfall_object(
    printings: &mut Printings,
    card_names: &ScryfallCardNames,
    object: &serde_json::Map<String, serde_json::Value>,
) {
    match ScryfallCard::from_scryfall_object(object) {
        Some(card) => insert_scryfall_card(printings, card_names, card),
        None => error!("couldn't convert scryfall object {:?}", object),
    }
}

pub async fn query_scryfall_by_name(
    name: &str,
    client: &ScryfallClient,
) -> Option<Vec<serde_json::Map<String, serde_json::Value>>> {
    let uri = format!(
        "https://api.scryfall.com/cards/search?q=name=!{}&unique=prints",
        encode_card_name(name)
    );
    let request = client.call(&uri).await;
    match request {
        Ok(response) => match response.json::<ScryfallSearchAnswer>().await {
            Ok(answer) => Some(answer.data),
            Err(deserialization_error) => {
                info!(
                    "error in deserializing scryfall search request by name: {}",
                    deserialization_error
                );
                None
            }
        },
        Err(e) => {
            info!("error in scryfall search request by name: {}", e);
            None
        }
    }
}

// pub async fn query_scryfall_object(
//     name: &str,
//     set: Option<&str>,
//     client: &ScryfallClient,
// ) -> Option<serde_json::Map<String, serde_json::Value>> {
//     let mut uri = format!(
//         "https://api.scryfall.com/cards/named?exact={}&format=json",
//         encode_card_name(name)
//     );
//     if set.is_some() {
//         uri += format!("&set={}", set.as_ref().unwrap()).as_str();
//     }
//     let request = client.call(&uri).await;
//     match request {
//         Ok(response) => match response
//             .json::<serde_json::Map<String, serde_json::Value>>()
//             .await
//         {
//             Ok(object) => Some(object),
//             Err(deserialization_error) => {
//                 info!(
//                     "error in deserialization of scryfall response: {}",
//                     deserialization_error
//                 );
//                 None
//             }
//         },
//         Err(request_error) => {
//             info!("error in call to scryfall api: {}", request_error);
//             None
//         }
//     }
// }
