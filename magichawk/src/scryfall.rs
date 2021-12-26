extern crate reqwest;
extern crate tokio;

use std::sync::Mutex;
use tokio::time::{Duration, Instant};

const SCRYFALL_COOLDOWN: Duration = Duration::from_millis(100);
pub struct ScryfallClient {
    client: reqwest::Client,
    last_call: Mutex<Instant>,
}

impl ScryfallClient {
    pub fn new() -> ScryfallClient {
        ScryfallClient {
            client: reqwest::Client::new(),
            last_call: Mutex::new(Instant::now() - SCRYFALL_COOLDOWN),
        }
    }

    pub async fn call(&self, uri: &str) -> Result<reqwest::Response, reqwest::Error> {
        let next_call = (|| {
            let mut l = *self.last_call.lock().unwrap();
            l += SCRYFALL_COOLDOWN;
            l
        })();
        tokio::time::sleep_until(next_call).await;
        self.client.get(uri).send().await
    }
}
