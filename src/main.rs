#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
extern crate rocket_contrib;

use rocket::{http::ContentType, response::Content, Response, State};
use rocket_contrib::serve::StaticFiles;
use std::sync::Mutex;
use std::time::Duration;

#[get("/decklist?<text>")]
fn process_decklist(
    state: State<Mutex<magichawk::ScryfallCache>>,
    text: String,
) -> Response<'static> {
    match magichawk::decklist_to_pdf(&mut state.lock().unwrap(), &text) {
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
    age_seconds: Option<u64>,
) -> Content<String> {
    state
        .lock()
        .unwrap()
        .purge(age_seconds.map(|s| Duration::from_secs(s)));
    list_cache(state)
}

fn main() {
    let cache = Mutex::new(magichawk::ScryfallCache::new());

    magichawk::setup_logger().unwrap();
    rocket::ignite()
        .mount("/", StaticFiles::from("static/"))
        .mount("/", routes![process_decklist])
        .mount("/", routes![list_cache])
        .mount("/", routes![purge_cache])
        .manage(cache)
        .launch();
}
