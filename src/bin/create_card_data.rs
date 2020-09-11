use clap::Clap;
use magichawk::{CardPrinting, ScryfallCard, ScryfallCardNames, SCRYFALL_CARD_NAMES};
use serde_json::{from_reader, Value};
use std::collections::HashMap;
use std::fs::File;

/// Process a bulk "Default Cards" file from the Scryfall API into a card data file for magichawk
/// see https://scryfall.com/docs/api/bulk-data for the bulk data
#[derive(Clap)]
#[clap(version = "0.1")]
struct Opts {
    /// bulk "Default Cards" input filename
    input: String,
    /// output filename
    output: String,
}

fn main() {
    let opts: Opts = Opts::parse();

    let nontoken_names: ScryfallCardNames =
        serde_json::from_reader(reqwest::blocking::get(SCRYFALL_CARD_NAMES).unwrap()).unwrap();
    println!(
        "there are {} card names, the first is {}",
        nontoken_names.names.len(),
        nontoken_names.names[0]
    );

    let f = File::open(&opts.input).unwrap();
    let outputfile = File::create(opts.output).unwrap();

    let default_cards: Vec<serde_json::Map<String, Value>> = from_reader(f).unwrap();
    println!(
        "there are {} entries in {}",
        default_cards.len(),
        &opts.input
    );
    // println!("{}", serde_json::to_string_pretty(&default_cards).unwrap());

    let mut card_data: HashMap<String, Vec<CardPrinting>> = HashMap::new();

    for default_card in default_cards {
        let sc = ScryfallCard::from_scryfall_object(default_card);
        match sc {
            Some(card) => {
                println!(
                    "scryfall card: {}",
                    serde_json::to_string_pretty(&card).unwrap()
                );
                if nontoken_names.names.contains(&card.name) {
                    println!("found name {}, adding", card.name);
                    card_data
                        .entry(card.name)
                        .or_insert(Vec::new())
                        .push(card.printing);
                }
            }

            None => println!("couldn't parse to ScryfallCard"),
        }
    }

    serde_json::to_writer(outputfile, &card_data).unwrap();
}
