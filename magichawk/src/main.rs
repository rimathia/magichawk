#[macro_use]
extern crate rocket;
extern crate serde_json;

use itertools::Itertools;
use printpdf::image_crate::DynamicImage;
use rocket::http::{ContentType, Status};
use rocket::{State, fairing::AdHoc, response::content};
use std::fs::File;
use tokio::sync::Mutex;

use magichawk::ScryfallClient;

#[get("/")]
async fn get_index() -> content::RawHtml<String> {
    let index = include_str!("../static/index.html");
    content::RawHtml(index.into())
}

#[get("/create_pdf?<decklist>&<backside>")]
async fn create_pdf(
    image_cache: &State<Mutex<magichawk::ScryfallCache>>,
    card_data: &State<Mutex<magichawk::CardData>>,
    client: &State<ScryfallClient>,
    decklist: String,
    backside: magichawk::BacksideMode,
) -> (rocket::http::Status, (rocket::http::ContentType, Vec<u8>)) {
    let mut cd = card_data.lock().await;
    let parsed = magichawk::parse_decklist(&decklist, &cd.printings.languages);
    let cards = magichawk::image_lines_from_decklist(parsed, &mut cd, backside, client).await;

    let mut cache = image_cache.lock().await;
    for line in cards.iter() {
        cache.ensure_contains_line(line, client).await;
    }

    let mut expanded: Vec<&DynamicImage> = Vec::new();
    for line in cards.iter() {
        for (uri, multiplicity) in &line.images {
            if let Some(image) = cache.get(uri) {
                for _i in 0..*multiplicity {
                    expanded.push(image);
                }
            }
        }
    }

    if expanded.is_empty() {
        let message: Vec<u8> = "no card names have been recognized".as_bytes().to_vec();
        return (Status::BadRequest, (ContentType::Plain, message));
    }

    let pdf = magichawk::page_images_to_pdf(
        expanded
            .into_iter()
            .batching(|it| magichawk::images_to_page(it)),
    );

    match pdf {
        Some(bytes) => {
            info!("sending out pdf with size {}", bytes.len());
            (Status::Ok, (ContentType::PDF, bytes))
        }
        None => {
            let message: Vec<u8> = "internal server error (sorry)".as_bytes().to_vec();
            (Status::InternalServerError, (ContentType::Plain, message))
        }
    }
}

#[get("/cache/list")]
async fn list_cache(state: &State<Mutex<magichawk::ScryfallCache>>) -> content::RawHtml<String> {
    content::RawHtml(state.lock().await.list())
}

#[get("/cache/purge?<age_seconds>")]
async fn purge_cache(
    state: &State<Mutex<magichawk::ScryfallCache>>,
    age_seconds: Option<i64>,
) -> content::RawHtml<String> {
    state
        .lock()
        .await
        .purge(age_seconds.map(time::Duration::seconds));
    list_cache(state).await
}

#[get("/card_names/full")]
async fn card_names_full(
    card_data_m: &State<Mutex<magichawk::CardData>>,
) -> content::RawJson<String> {
    let card_names = &card_data_m.lock().await.card_names;
    let serialized: String = serde_json::to_string_pretty(card_names).unwrap();
    content::RawJson(serialized)
}

#[get("/card_names/short")]
async fn card_names_short(
    card_data_m: &State<Mutex<magichawk::CardData>>,
) -> content::RawHtml<String> {
    let card_names = &card_data_m.lock().await.card_names;
    let names = &card_names.names;
    let update: String = match card_names.date {
        Some(date) => format!("{}", date),
        None => "not present (this indicates a bug)".to_string(),
    };
    content::RawHtml(format!(
        "There are {} card names, last update approximately {}, the first three are {:?}, {:?}, {:?}",
        names.len(),
        update,
        names.first(),
        names.get(1),
        names.get(2),
    ))
}

#[get("/card_names/update")]
async fn card_names_update(
    card_data_m: &State<Mutex<magichawk::CardData>>,
    client: &State<ScryfallClient>,
) -> content::RawHtml<String> {
    let mut card_data = card_data_m.lock().await;
    let n_before = card_data.card_names.names.len();
    let response = match card_data.update_names(client).await {
        Some(_) => {
            let n_after = card_data.card_names.names.len();
            format!(
                "card names updated, {} names before, {} names after",
                n_before, n_after
            )
        }
        None => "couldn't update card names".to_string(),
    };
    content::RawHtml(response)
}

#[get("/lookup")]
async fn lookup(card_data_m: &State<Mutex<magichawk::CardData>>) -> content::RawHtml<String> {
    let lookup = &card_data_m.lock().await.lookup;
    content::RawHtml(format!("{:?}", lookup))
}

#[get("/card_data/short")]
async fn card_data_short(
    card_data_m: &State<Mutex<magichawk::CardData>>,
) -> content::RawHtml<String> {
    let card_data = card_data_m.lock().await;
    content::RawHtml(format!(
        "There are {} different card names and {} (card name, set) combinations",
        card_data.printings.printings.len(),
        card_data
            .printings
            .printings
            .values()
            .map(|printings| printings.len())
            .sum::<usize>()
    ))
}

#[get("/card_data/full")]
async fn card_data_full(
    card_data_m: &State<Mutex<magichawk::CardData>>,
) -> content::RawJson<String> {
    let card_data = card_data_m.lock().await;
    let serialized: String = serde_json::to_string_pretty(&card_data.printings.printings).unwrap();
    content::RawJson(serialized)
}

#[derive(Debug, rocket::serde::Deserialize)]
struct AppConfig {
    card_data: Option<String>,
}

async fn trigger_local_call(name: String, url: String, interval: std::time::Duration) {
    let client = reqwest::Client::new();
    let mut wakeup_time = tokio::time::Instant::now() + interval;
    loop {
        tokio::time::sleep_until(wakeup_time).await;
        wakeup_time += interval;
        info!("trigger update of card names");
        match client.get(&url).send().await {
            Ok(_response) => {
                info!("response to local call {} ok", name);
            }
            Err(e) => {
                error!("error when trying local call {}: {}", name, e);
            }
        }
    }
}

async fn trigger_card_name_update() {
    trigger_local_call(
        "card name update".to_string(),
        "http://localhost:8000/card_names/update".to_string(),
        std::time::Duration::from_secs(10 * 60),
    )
    .await
}

async fn trigger_cache_purge() {
    trigger_local_call(
        "purge cache".to_string(),
        "http://localhost:8000/cache/purge".to_string(),
        std::time::Duration::from_secs(24 * 60 * 60),
    )
    .await
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .attach(AdHoc::config::<AppConfig>())
        .attach(AdHoc::on_liftoff(
            "create trigger for update of card names",
            |_| {
                Box::pin(async move {
                    tokio::task::spawn(trigger_card_name_update());
                })
            },
        ))
        .attach(AdHoc::on_liftoff(
            "create trigger for purging cache",
            |_| {
                Box::pin(async move {
                    tokio::task::spawn(trigger_cache_purge());
                })
            },
        ))
        .attach(AdHoc::on_ignite("create reqwest client", |rocket| async {
            rocket.manage(ScryfallClient::new())
        }))
        .attach(AdHoc::on_ignite(
            "load card data from file",
            |rocket| async {
                let client = rocket
                    .state::<ScryfallClient>()
                    .expect("we should always be able to get a scryfall client");
                let card_data_from_file: Option<magichawk::CardData> = async {
                    let file_name = rocket.state::<AppConfig>().unwrap().card_data.clone();
                    let file_handle = File::open(file_name?).ok()?;
                    let deserialized: magichawk::CardPrintings =
                        serde_json::from_reader(file_handle)
                            .ok()
                            .unwrap_or_else(magichawk::get_minimal_card_printings);

                    magichawk::CardData::from_printings(deserialized, client).await
                }
                .await;
                let card_data = card_data_from_file.unwrap_or(
                    magichawk::CardData::from_client(client).await.expect(
                        "we should always be able to create card data from the scryfall client",
                    ),
                );
                rocket.manage(Mutex::new(card_data))
            },
        ))
        .attach(AdHoc::on_ignite("create image cache", |rocket| async {
            rocket.manage(Mutex::new(magichawk::ScryfallCache::new()))
        }))
        // .mount("/", FileServer::from(Path::new("static")))
        .mount("/", routes![get_index])
        .mount("/", routes![card_names_full])
        .mount("/", routes![card_names_short])
        .mount("/", routes![card_names_update])
        .mount("/", routes![card_data_full])
        .mount("/", routes![card_data_short])
        .mount("/", routes![lookup])
        .mount("/", routes![create_pdf])
        .mount("/", routes![list_cache])
        .mount("/", routes![purge_cache])
}
