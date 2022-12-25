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

// the uri of the border crop for a meld result is not part of the scryfall object
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ScryfallObjectBack {
    Uri(String),
    MeldResultName(String),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ScryfallObject {
    pub name: String,
    pub set: String,
    pub border_crop: String,
    pub border_crop_back: Option<ScryfallObjectBack>,
}

impl ScryfallObject {
    pub fn from_dict(d: &serde_json::Map<String, serde_json::Value>) -> Option<ScryfallObject> {
        let n: String = d["name"].as_str()?.to_string().to_lowercase();
        let s = d["set"].as_str()?.to_string().to_lowercase();
        let (bc, bcb) = {
            if d.contains_key("card_faces") {
                let card_faces = d["card_faces"].as_array()?;
                if card_faces.len() != 2 {
                    return None;
                } else {
                    (
                        card_faces[0]["image_uris"]["border_crop"]
                            .as_str()?
                            .to_string(),
                        Some(ScryfallObjectBack::Uri(
                            card_faces[1]["image_uris"]["border_crop"]
                                .as_str()?
                                .to_string(),
                        )),
                    )
                }
            } else if d.contains_key("image_uris") {
                let front = d["image_uris"]["border_crop"].as_str()?.to_string();
                let back = if d["layout"] == "meld" {
                    let all_parts = &d["all_parts"];
                    match all_parts {
                        serde_json::Value::Array(a) => {
                            let meld_result =
                                a.iter().find(|entry| entry["component"] == "meld_result")?["name"]
                                    .as_str()?
                                    .to_lowercase();
                            Some(ScryfallObjectBack::MeldResultName(meld_result.to_string()))
                        }
                        _ => None,
                    }
                } else {
                    None
                };
                (front, back)
            } else {
                return None;
            }
        };
        Some(ScryfallObject {
            name: n,
            set: s,
            border_crop: bc,
            border_crop_back: bcb,
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CardPrinting {
    pub set: String,
    pub border_crop: String,
    pub border_crop_back: Option<String>,
}

pub type CardPrintings = HashMap<String, Vec<CardPrinting>>;

#[derive(Serialize, Deserialize, Debug)]
pub struct Card {
    pub name: String,
    pub printing: CardPrinting,
}

impl Card {
    pub fn from_scryfall_object(_d: &serde_json::Map<String, serde_json::Value>) -> Option<Card> {
        None
        // let n: String = d["name"].as_str()?.to_string().to_lowercase();
        // let s = d["set"].as_str()?.to_string().to_lowercase();
        // let (bc, bcb) = {
        //     if d.contains_key("card_faces") {
        //         let card_faces = d["card_faces"].as_array()?;
        //         if card_faces.len() != 2 {
        //             return None;
        //         } else {
        //             (
        //                 card_faces[0]["image_uris"]["border_crop"]
        //                     .as_str()?
        //                     .to_string(),
        //                 Some(CardPrintingBack::Url(
        //                     card_faces[1]["image_uris"]["border_crop"]
        //                         .as_str()?
        //                         .to_string(),
        //                 )),
        //             )
        //         }
        //     } else if d.contains_key("image_uris") {
        //         let front = d["image_uris"]["border_crop"].as_str()?.to_string();
        //         let back = if (d["layout"] == "meld") {
        //             let all_parts = d["all_parts"].as_array()?;
        //             let meld_result = all_parts
        //                 .iter()
        //                 .find(|entry| entry["component"] == "meld_result")?
        //                 .as_str()?;
        //             Some(CardPrintingBack::MeldResultName(meld_result.to_string()))
        //         } else {
        //             None
        //         };
        //         (front, back)
        //     } else {
        //         return None;
        //     }
        // };
        // Some(Card {
        //     name: n,
        //     printing: CardPrinting {
        //         set: s,
        //         border_crop: bc,
        //         border_crop_back: bcb,
        //     },
        // })
    }
}

pub fn insert_scryfall_card(
    printings: &mut CardPrintings,
    card_names: &ScryfallCardNames,
    card: Card,
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
    printings: &mut CardPrintings,
    card_names: &ScryfallCardNames,
    object: &serde_json::Map<String, serde_json::Value>,
) {
    match Card::from_scryfall_object(object) {
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meld_result() {
        let urza_lord_protector = r#"{"object":"card","id":"8aefe8bd-216a-4ec1-9362-3f9dbf7fd083","oracle_id":"df2af646-3e5b-43a3-8f3e-50565889f456","multiverse_ids":[588288],"mtgo_id":105072,"arena_id":82710,"tcgplayer_id":448412,"cardmarket_id":678194,"name":"Urza, Lord Protector","lang":"en","released_at":"2022-11-18","uri":"https://api.scryfall.com/cards/8aefe8bd-216a-4ec1-9362-3f9dbf7fd083","scryfall_uri":"https://scryfall.com/card/bro/225/urza-lord-protector?utm_source=api","layout":"meld","highres_image":false,"image_status":"lowres","image_uris":{"small":"https://cards.scryfall.io/small/front/8/a/8aefe8bd-216a-4ec1-9362-3f9dbf7fd083.jpg?1670539417","normal":"https://cards.scryfall.io/normal/front/8/a/8aefe8bd-216a-4ec1-9362-3f9dbf7fd083.jpg?1670539417","large":"https://cards.scryfall.io/large/front/8/a/8aefe8bd-216a-4ec1-9362-3f9dbf7fd083.jpg?1670539417","png":"https://cards.scryfall.io/png/front/8/a/8aefe8bd-216a-4ec1-9362-3f9dbf7fd083.png?1670539417","art_crop":"https://cards.scryfall.io/art_crop/front/8/a/8aefe8bd-216a-4ec1-9362-3f9dbf7fd083.jpg?1670539417","border_crop":"https://cards.scryfall.io/border_crop/front/8/a/8aefe8bd-216a-4ec1-9362-3f9dbf7fd083.jpg?1670539417"},"mana_cost":"{1}{W}{U}","cmc":3.0,"type_line":"Legendary Creature — Human Artificer","oracle_text":"Artifact, instant, and sorcery spells you cast cost {1} less to cast.\n{7}: If you both own and control Urza, Lord Protector and an artifact named The Mightstone and Weakstone, exile them, then meld them into Urza, Planeswalker. Activate only as a sorcery.","power":"2","toughness":"4","colors":["U","W"],"color_identity":["U","W"],"keywords":["Meld"],"all_parts":[{"object":"related_card","id":"40a01679-3224-427e-bd1d-b797b0ab68b7","component":"meld_result","name":"Urza, Planeswalker","type_line":"Legendary Planeswalker — Urza","uri":"https://api.scryfall.com/cards/40a01679-3224-427e-bd1d-b797b0ab68b7"},{"object":"related_card","id":"02aea379-b444-46a3-82f4-3038f698d4f4","component":"meld_part","name":"The Mightstone and Weakstone","type_line":"Legendary Artifact — Powerstone","uri":"https://api.scryfall.com/cards/02aea379-b444-46a3-82f4-3038f698d4f4"},{"object":"related_card","id":"8aefe8bd-216a-4ec1-9362-3f9dbf7fd083","component":"meld_part","name":"Urza, Lord Protector","type_line":"Legendary Creature — Human Artificer","uri":"https://api.scryfall.com/cards/8aefe8bd-216a-4ec1-9362-3f9dbf7fd083"}],"legalities":{"standard":"legal","future":"legal","historic":"legal","gladiator":"legal","pioneer":"legal","explorer":"legal","modern":"legal","legacy":"legal","pauper":"not_legal","vintage":"legal","penny":"not_legal","commander":"legal","brawl":"legal","historicbrawl":"legal","alchemy":"legal","paupercommander":"not_legal","duel":"legal","oldschool":"not_legal","premodern":"not_legal"},"games":["paper","mtgo","arena"],"reserved":false,"foil":true,"nonfoil":true,"finishes":["nonfoil","foil"],"oversized":false,"promo":false,"reprint":false,"variation":false,"set_id":"4219a14e-6701-4ddd-a185-21dc054ab19b","set":"bro","set_name":"The Brothers' War","set_type":"expansion","set_uri":"https://api.scryfall.com/sets/4219a14e-6701-4ddd-a185-21dc054ab19b","set_search_uri":"https://api.scryfall.com/cards/search?order=set\u0026q=e%3Abro\u0026unique=prints","scryfall_set_uri":"https://scryfall.com/sets/bro?utm_source=api","rulings_uri":"https://api.scryfall.com/cards/8aefe8bd-216a-4ec1-9362-3f9dbf7fd083/rulings","prints_search_uri":"https://api.scryfall.com/cards/search?order=released\u0026q=oracleid%3Adf2af646-3e5b-43a3-8f3e-50565889f456\u0026unique=prints","collector_number":"225","digital":false,"rarity":"mythic","card_back_id":"58a4215b-9f3d-40d4-bc05-d8d3cc2354d9","artist":"Ryan Pancoast","artist_ids":["89cc9475-dda2-4d13-bf88-54b92867a25c"],"illustration_id":"c1abe983-d141-4884-9812-2593773f1a59","border_color":"black","frame":"2015","frame_effects":["legendary"],"security_stamp":"oval","full_art":false,"textless":false,"booster":true,"story_spotlight":false,"edhrec_rank":7316,"prices":{"usd":"26.65","usd_foil":"31.39","usd_etched":null,"eur":"19.24","eur_foil":"29.19","tix":"5.82"},"related_uris":{"gatherer":"https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=588288","tcgplayer_infinite_articles":"https://infinite.tcgplayer.com/search?contentMode=article\u0026game=magic\u0026partner=scryfall\u0026q=Urza%2C+Lord+Protector\u0026utm_campaign=affiliate\u0026utm_medium=api\u0026utm_source=scryfall","tcgplayer_infinite_decks":"https://infinite.tcgplayer.com/search?contentMode=deck\u0026game=magic\u0026partner=scryfall\u0026q=Urza%2C+Lord+Protector\u0026utm_campaign=affiliate\u0026utm_medium=api\u0026utm_source=scryfall","edhrec":"https://edhrec.com/route/?cc=Urza%2C+Lord+Protector"},"purchase_uris":{"tcgplayer":"https://www.tcgplayer.com/product/448412?page=1\u0026utm_campaign=affiliate\u0026utm_medium=api\u0026utm_source=scryfall","cardmarket":"https://www.cardmarket.com/en/Magic/Products/Search?referrer=scryfall\u0026searchString=Urza%2C+Lord+Protector\u0026utm_campaign=card_prices\u0026utm_medium=text\u0026utm_source=scryfall","cardhoarder":"https://www.cardhoarder.com/cards/105072?affiliate_id=scryfall\u0026ref=card-profile\u0026utm_campaign=affiliate\u0026utm_medium=card\u0026utm_source=scryfall"}}"#;
        let v: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(urza_lord_protector).unwrap();
        let object = ScryfallObject::from_dict(&v).unwrap();
        print!("{:?}", object);
        assert_eq!(
            object.border_crop_back,
            Some(ScryfallObjectBack::MeldResultName(
                "Urza, Planeswalker".to_string()
            ))
        );
    }
}
