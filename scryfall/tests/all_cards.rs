use std::fs::File;
use std::path::Path;

use scryfall::Card;

#[test]
fn test_all_cards() {
    let f = File::open(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("bulk/all-cards-20211228221212.json"),
    )
    .expect("couldn't open input file with all cards");

    let cards: Vec<Card> = serde_json::from_reader(f).unwrap();
    println!("there are {} cards in the all-cards bulk file", cards.len());
}
