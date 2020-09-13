#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
extern crate rocket_contrib;
extern crate serde_json;

use image::DynamicImage;
use itertools::Itertools;
use rocket::{fairing::AdHoc, http::ContentType, response::Content, Response, State};
use rocket_contrib::serve::StaticFiles;
use std::fs::File;
use std::sync::Mutex;
use std::thread;

#[get("/create_pdf?<decklist>&<backside>")]
fn create_pdf(
    image_cache: State<Mutex<magichawk::ScryfallCache>>,
    card_data: State<Mutex<magichawk::CardData>>,
    decklist: String,
    backside: magichawk::BacksideMode,
) -> Response<'static> {
    let parsed = magichawk::parse_decklist(&decklist);
    let lookedup: Vec<magichawk::DecklistEntry> = parsed
        .iter()
        .filter_map(|line| {
            let entry = line.as_entry()?;
            card_data.lock().unwrap().ensure_contains(&entry.card.name);
            Some(entry)
        })
        .collect();
    let mut cache = image_cache.lock().unwrap();
    let uris: Vec<(&magichawk::DecklistEntry, String, Option<String>)> = lookedup
        .iter()
        .filter_map(|entry| {
            let mut uris = card_data.lock().unwrap().get_uris(
                &entry.card.name,
                entry.card.set.as_ref().map(|s| s.as_str()),
            )?;
            cache.ensure_contains(&uris.0)?;
            uris.1 = if backside != magichawk::BacksideMode::Zero {
                match uris.1 {
                    Some(ref back) => match cache.ensure_contains(back) {
                        Some(_) => uris.1,
                        None => None,
                    },
                    None => None,
                }
            } else {
                None
            };
            Some((entry, uris.0, uris.1))
        })
        .collect();

    let mut expanded: Vec<&DynamicImage> = Vec::new();
    for (entry, front, maybe_back) in uris.iter() {
        match cache.get(front) {
            Some(image) => {
                for _i in 0..entry.multiple {
                    expanded.push(image);
                }
            }
            None => {}
        }
        match maybe_back {
            Some(back) => match cache.get(back) {
                Some(image) => match backside {
                    magichawk::BacksideMode::Zero => {}
                    magichawk::BacksideMode::One => expanded.push(image),
                    magichawk::BacksideMode::Matching => {
                        for _i in 0..entry.multiple {
                            expanded.push(image);
                        }
                    }
                },
                None => {}
            },
            None => {}
        }
    }

    let pdf = magichawk::pages_to_pdf(
        expanded
            .into_iter()
            .batching(|it| magichawk::images_to_page(it)),
    );

    //Response::build()
    //    .header(ContentType::HTML)
    //    .sized_body(Cursor::new(format!(
    //        "parsed: {:?}\nbackside: {:?}\nlookedup: {:?}\nuris: {:?}",
    //        parsed, backside, lookedup, uris
    //    )))
    //    .finalize()
    match pdf {
        Some(bytes) => Response::build()
            .header(ContentType::PDF)
            .sized_body(std::io::Cursor::new(bytes))
            .finalize(),
        None => Response::build().header(ContentType::HTML).finalize(),
    }
}

#[get("/cache/list")]
fn list_cache(state: State<Mutex<magichawk::ScryfallCache>>) -> Content<String> {
    Content(ContentType::HTML, state.lock().unwrap().list())
}

#[get("/cache/purge?<age_seconds>")]
fn purge_cache(
    state: State<Mutex<magichawk::ScryfallCache>>,
    age_seconds: Option<i64>,
) -> Content<String> {
    state
        .lock()
        .unwrap()
        .purge(age_seconds.map(|s| chrono::Duration::seconds(s)));
    list_cache(state)
}

#[get("/card_names/full")]
fn card_names_full(card_data_m: State<Mutex<magichawk::CardData>>) -> Content<String> {
    let card_names = &card_data_m.lock().unwrap().card_names;
    Content(
        ContentType::JSON,
        serde_json::to_string(card_names).unwrap(),
    )
}

#[get("/card_names/short")]
fn card_names_short(card_data_m: State<Mutex<magichawk::CardData>>) -> Content<String> {
    let names = &card_data_m.lock().unwrap().card_names.names;
    Content(
        ContentType::HTML,
        format!(
            "There are {} card names, the first three are {:?}, {:?}, {:?}",
            names.len(),
            names.get(0),
            names.get(1),
            names.get(2),
        ),
    )
}

#[get("/card_names/update")]
fn card_names_update(card_data_m: State<Mutex<magichawk::CardData>>) -> Content<String> {
    Content(
        ContentType::HTML,
        match card_data_m.lock().unwrap().update_names() {
            Some(_) => "card names updated".to_string(),
            None => "couldn't update card names".to_string(),
        },
    )
}

#[get("/lookup")]
fn lookup(card_data_m: State<Mutex<magichawk::CardData>>) -> Content<String> {
    let lookup = &card_data_m.lock().unwrap().lookup;
    Content(ContentType::HTML, format!("{:?}", lookup))
}

#[get("/card_data/short")]
fn card_data_short(card_data_m: State<Mutex<magichawk::CardData>>) -> Content<String> {
    let card_data = card_data_m.lock().unwrap();
    Content(
        ContentType::HTML,
        format!(
            "There are {} different card names and {} (card name, set) combinations",
            card_data.printings.len(),
            card_data
                .printings
                .iter()
                .map(|(_name, printings)| printings.len())
                .sum::<usize>()
        ),
    )
}

#[get("/card_data/full")]
fn card_data_full(card_data_m: State<Mutex<magichawk::CardData>>) -> Content<String> {
    let card_data = card_data_m.lock().unwrap();
    Content(
        ContentType::JSON,
        serde_json::to_string(&card_data.printings).unwrap(),
    )
}
fn main() {
    thread::spawn(|| loop {
        let local_query = reqwest::blocking::get("http://localhost:8000/card_names/update");
        match local_query {
            Ok(response) => println!("local response to card updates: {:?}", response.text()),
            Err(e) => println!("error for local query for card update: {}", e),
        }
        std::thread::sleep(std::time::Duration::from_secs(10 * 60));
    });
    magichawk::setup_logger().unwrap();
    rocket::ignite()
        .attach(AdHoc::on_attach("load card data from file", |rocket| {
            let bulk: std::collections::HashMap<String, Vec<magichawk::CardPrinting>> =
                serde_json::from_reader(
                    File::open(rocket.config().get_str("card_data").unwrap()).unwrap(),
                )
                .unwrap();
            Ok(rocket.manage(Mutex::new(magichawk::CardData::from_bulk(bulk).unwrap())))
        }))
        .attach(AdHoc::on_attach("create image cache", |rocket| {
            Ok(rocket.manage(Mutex::new(magichawk::ScryfallCache::new())))
        }))
        .mount("/", StaticFiles::from("static/"))
        .mount("/", routes![card_names_full])
        .mount("/", routes![card_names_short])
        .mount("/", routes![card_names_update])
        .mount("/", routes![card_data_full])
        .mount("/", routes![card_data_short])
        .mount("/", routes![lookup])
        .mount("/", routes![create_pdf])
        .mount("/", routes![list_cache])
        .mount("/", routes![purge_cache])
        .launch();
}
