#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
// #[macro_use]
extern crate rocket_contrib;

use rocket::{http::ContentType, Response, State};
use rocket_contrib::serve::StaticFiles;
use std::sync::Mutex;

#[get("/decklist?<text>")]
fn process_decklist(
    state: State<Mutex<magichawk::ScryfallCache>>,
    text: String,
) -> Response<'static> {
    match magichawk::decklist_to_zipped_tiff(&mut state.lock().unwrap(), &text) {
        Some(bytes) => Response::build()
            .header(ContentType::ZIP)
            .sized_body(std::io::Cursor::new(bytes))
            .finalize(),
        None => Response::build().header(ContentType::HTML).finalize(),
    }
}

fn main() {
    let cache = Mutex::new(magichawk::ScryfallCache::new());

    magichawk::setup_logger().unwrap();
    rocket::ignite()
        .mount("/", StaticFiles::from("static/"))
        .mount("/", routes![process_decklist])
        .manage(cache)
        .launch();
}
