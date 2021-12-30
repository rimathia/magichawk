extern crate reqwest;
extern crate tokio;

use lazy_static::lazy_static;
use log::debug;
use tokio::time::{Duration, Instant};

const SCRYFALL_COOLDOWN: Duration = Duration::from_millis(100);

// use a blocking mutex since we are only holding the lock to find out when we can call
lazy_static! {
    static ref LAST_SCRYFALL_CALL: std::sync::Mutex<Instant> =
        std::sync::Mutex::new(Instant::now() - SCRYFALL_COOLDOWN);
}
pub struct ScryfallClient {
    client: reqwest::Client,
}

impl ScryfallClient {
    pub fn new() -> ScryfallClient {
        ScryfallClient {
            client: reqwest::Client::new(),
        }
    }

    pub async fn call(&self, uri: &str) -> Result<reqwest::Response, reqwest::Error> {
        let next_call = {
            let mut l = *LAST_SCRYFALL_CALL.lock().unwrap();
            l += SCRYFALL_COOLDOWN;
            l
        };
        tokio::time::sleep_until(next_call).await;
        debug!("calling scryfall API: {}", uri);
        self.client.get(uri).send().await
    }
}

impl Default for ScryfallClient {
    fn default() -> Self {
        Self::new()
    }
}

pub fn blocking_call(uri: &str) -> Result<reqwest::blocking::Response, reqwest::Error> {
    let next_call = {
        let mut l = *LAST_SCRYFALL_CALL.lock().unwrap();
        l += SCRYFALL_COOLDOWN;
        l
    };
    let sleep_interval = next_call - Instant::now();
    if sleep_interval > Duration::from_secs(0) {
        std::thread::sleep(sleep_interval);
    }
    reqwest::blocking::get(uri)
}
