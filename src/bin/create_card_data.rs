use clap::Parser;
use magichawk::{CardPrinting, ScryfallCardNames};
use serde_json::{from_reader, Value};
use std::fs::File;

/// Process a bulk "Default Cards" file from the Scryfall API into a card data file for magichawk,
/// see https://scryfall.com/docs/api/bulk-data for the bulk data.
/// The bulk data list of different card names from the Scryfall API is used to exclude tokens.
#[derive(Parser, Debug)]
struct Opts {
    /// bulk "Default Cards" input filename
    input: String,
    /// output filename
    output: String,
}

fn main() {
    let opts = Opts::parse();

    let nontoken_names = ScryfallCardNames::from_api_call_blocking().unwrap();
    println!(
        "There are {} card names, the first is {}",
        nontoken_names.names.len(),
        nontoken_names.names[0]
    );

    let f = File::open(&opts.input).unwrap();
    let outputfile = File::create(&opts.output).unwrap();

    let default_cards: Vec<serde_json::Map<String, Value>> = from_reader(f).unwrap();
    println!(
        "there are {} entries in {}",
        default_cards.len(),
        &opts.input
    );

    let mut card_data: std::collections::HashMap<String, Vec<CardPrinting>> =
        std::collections::HashMap::new();
    for default_card in default_cards.iter() {
        magichawk::insert_scryfall_object(&mut card_data, &nontoken_names, default_card);
    }

    serde_json::to_writer(outputfile, &card_data).unwrap();

    let different_cards: usize = card_data
        .iter()
        .map(|(_name, printings)| printings.len())
        .sum();
    println!(
        "there are {} card names and {} (card name, set) combinations in {}",
        card_data.len(),
        different_cards,
        &opts.output
    );
}
