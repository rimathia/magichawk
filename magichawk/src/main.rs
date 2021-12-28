#[macro_use]
extern crate rocket;
extern crate serde_json;

use image::DynamicImage;
use itertools::Itertools;
use rocket::http::{ContentType, Status};
use rocket::{fairing::AdHoc, fs::FileServer, response::content, State};
use std::fs::File;
use std::path::Path;
use tokio::sync::Mutex;

use magichawk::ScryfallClient;

#[get("/create_pdf?<decklist>&<backside>")]
async fn create_pdf(
    image_cache: &State<Mutex<magichawk::ScryfallCache>>,
    card_data: &State<Mutex<magichawk::CardData>>,
    client: &State<ScryfallClient>,
    decklist: String,
    backside: magichawk::BacksideMode,
) -> (rocket::http::Status, (rocket::http::ContentType, Vec<u8>)) {
    let parsed = magichawk::parse_decklist(&decklist);
    let mut cd = card_data.lock().await;
    let cards = magichawk::image_lines_from_decklist(parsed, &mut cd, backside, client).await;

    let mut cache = image_cache.lock().await;
    for line in cards.iter() {
        cache.ensure_contains_line(line, client).await;
    }

    let mut expanded: Vec<&DynamicImage> = Vec::new();
    for line in cards.iter() {
        if line.front > 0 {
            match cache.get(&line.card.printing.border_crop) {
                Some(image) => {
                    for _i in 0..line.front {
                        expanded.push(image);
                    }
                }
                None => {}
            };
        }
        if line.back > 0 {
            match &line.card.printing.border_crop_back {
                Some(uri) => match cache.get(uri) {
                    Some(image) => {
                        for _i in 0..line.back {
                            expanded.push(image);
                        }
                    }
                    None => {}
                },
                None => {}
            };
        }
    }

    if expanded.len() == 0 {
        let message: Vec<u8> = "no card names have been recognized".as_bytes().to_vec();
        return (Status::BadRequest, (ContentType::Plain, message));
    }

    let pdf = magichawk::pages_to_pdf(
        expanded
            .into_iter()
            .batching(|it| magichawk::images_to_page(it)),
    );

    match pdf {
        Some(bytes) => {
            info!("sending out pdf with size {}", bytes.len());
            return (Status::Ok, (ContentType::PDF, bytes));
        }
        None => {
            let message: Vec<u8> = "internal server error (sorry)".as_bytes().to_vec();
            return (Status::InternalServerError, (ContentType::Plain, message));
        }
    }
}

#[get("/cache/list")]
async fn list_cache(state: &State<Mutex<magichawk::ScryfallCache>>) -> content::Html<String> {
    content::Html(state.lock().await.list())
}

#[get("/cache/purge?<age_seconds>")]
async fn purge_cache(
    state: &State<Mutex<magichawk::ScryfallCache>>,
    age_seconds: Option<i64>,
) -> content::Html<String> {
    state
        .lock()
        .await
        .purge(age_seconds.map(|s| chrono::Duration::seconds(s)));
    list_cache(state).await
}

#[get("/card_names/full")]
async fn card_names_full(card_data_m: &State<Mutex<magichawk::CardData>>) -> content::Json<String> {
    let card_names = &card_data_m.lock().await.card_names;
    let serialized: String = serde_json::to_string_pretty(card_names).unwrap();
    content::Json(serialized)
}

#[get("/card_names/short")]
async fn card_names_short(
    card_data_m: &State<Mutex<magichawk::CardData>>,
) -> content::Html<String> {
    let card_names = &card_data_m.lock().await.card_names;
    let names = &card_names.names;
    let update: String = match card_names.date {
        Some(date) => date.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        None => "not present (this indicates a bug)".to_string(),
    };
    content::Html(format!(
        "There are {} card names, last update approximately {}, the first three are {:?}, {:?}, {:?}",
        names.len(),
        update,
        names.get(0),
        names.get(1),
        names.get(2),
    ))
}

#[get("/card_names/update")]
async fn card_names_update(
    card_data_m: &State<Mutex<magichawk::CardData>>,
    client: &State<ScryfallClient>,
) -> content::Html<String> {
    content::Html(match card_data_m.lock().await.update_names(client).await {
        Some(_) => "card names updated".to_string(),
        None => "couldn't update card names".to_string(),
    })
}

#[get("/lookup")]
async fn lookup(card_data_m: &State<Mutex<magichawk::CardData>>) -> content::Html<String> {
    let lookup = &card_data_m.lock().await.lookup;
    content::Html(format!("{:?}", lookup))
}

#[get("/card_data/short")]
async fn card_data_short(card_data_m: &State<Mutex<magichawk::CardData>>) -> content::Html<String> {
    let card_data = card_data_m.lock().await;
    content::Html(format!(
        "There are {} different card names and {} (card name, set) combinations",
        card_data.printings.len(),
        card_data
            .printings
            .iter()
            .map(|(_name, printings)| printings.len())
            .sum::<usize>()
    ))
}

#[get("/card_data/full")]
async fn card_data_full(card_data_m: &State<Mutex<magichawk::CardData>>) -> content::Json<String> {
    let card_data = card_data_m.lock().await;
    let serialized: String = serde_json::to_string_pretty(&card_data.printings).unwrap();
    content::Json(serialized)
}

#[derive(Debug, rocket::serde::Deserialize)]
struct AppConfig {
    card_data: String,
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .attach(AdHoc::config::<AppConfig>())
        .attach(AdHoc::on_liftoff(
            "create trigger for update of card names",
            |_| {
                Box::pin(async move {
                    tokio::task::spawn((|| async {
                        let client = reqwest::Client::new();
                        let interval = std::time::Duration::from_secs(10 * 60);
                        let mut wakeup_time = tokio::time::Instant::now() + interval;
                        loop {
                            tokio::time::sleep_until(wakeup_time).await;
                            wakeup_time += interval;
                            info!("trigger update of card names");
                            match client
                                .get("http://localhost:8000/card_names/update")
                                .send()
                                .await
                            {
                                Ok(_response) => {
                                    info!("response to triggering card name update ok");
                                }
                                Err(e) => {
                                    error!("error when trying to trigger card name update: {}", e);
                                }
                            }
                        }
                    })());
                })
            },
        ))
        .attach(AdHoc::on_liftoff(
            "create trigger for purging cache",
            |_| {
                Box::pin(async move {
                    tokio::task::spawn((|| async {
                        let client = reqwest::Client::new();
                        let interval = std::time::Duration::from_secs(24 * 60 * 60);
                        let mut wakeup_time = tokio::time::Instant::now() + interval;
                        loop {
                            tokio::time::sleep_until(wakeup_time).await;
                            wakeup_time += interval;
                            info!("trigger cache purge");
                            match client.get("http://localhost:8000/cache/purge").send().await {
                                Ok(_response) => {
                                    info!("response to triggering cache purge ok");
                                }
                                Err(e) => {
                                    error!("error when trying to trigger cache purge: {}", e);
                                }
                            }
                        }
                    })());
                })
            },
        ))
        .attach(AdHoc::on_ignite("create reqwest client", |rocket| async {
            rocket.manage(ScryfallClient::new())
        }))
        .attach(AdHoc::on_ignite(
            "load card data from file",
            |rocket| async {
                let file_name = rocket.state::<AppConfig>().unwrap().card_data.clone();
                let file_handle = File::open(file_name).unwrap();
                let bulk: magichawk::Printings = serde_json::from_reader(file_handle).unwrap();

                let client = rocket.state::<ScryfallClient>().unwrap();
                let card_data = magichawk::CardData::from_bulk(bulk, client).await.unwrap();
                rocket.manage(Mutex::new(card_data))
            },
        ))
        .attach(AdHoc::on_ignite("create image cache", |rocket| async {
            rocket.manage(Mutex::new(magichawk::ScryfallCache::new()))
        }))
        .mount("/", FileServer::from(Path::new("static")))
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
