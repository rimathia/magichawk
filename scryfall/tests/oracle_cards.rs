use std::fs::File;
use std::path::Path;

use scryfall::Card;

#[test]
fn test_oracle_cards() {
    let f = File::open(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("bulk/oracle-cards-20211228100345.json"),
    )
    .expect("couldn't open input file with oracle cards");

    let cards: Vec<Card> = serde_json::from_reader(f).unwrap();
    println!("there are {} cards", cards.len());
}
