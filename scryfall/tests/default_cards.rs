use std::fs::File;
use std::path::Path;

use scryfall::Card;

#[test]
fn test_default_cards() {
    let f = File::open(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("bulk/default-cards-20211228220250.json"),
    )
    .expect("couldn't open input file with default cards");

    let cards: Vec<Card> = serde_json::from_reader(f).unwrap();
    println!(
        "there are {} cards in the default-cards bulk file",
        cards.len()
    );
}
