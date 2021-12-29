use std::fs::File;
use std::path::Path;

use scryfall::Card;

#[test]
fn test_unique_artwork() {
    let f = File::open(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("bulk/unique-artwork-20211228221348.json"),
    )
    .expect("couldn't open input file with unique artwork cards");

    let cards: Vec<Card> = serde_json::from_reader(f).unwrap();
    println!(
        "there are {} cards in the unique-artwork bulk file",
        cards.len()
    );
}
