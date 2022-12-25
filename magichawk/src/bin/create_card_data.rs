use clap::Parser;
use magichawk::{CardPrinting, ScryfallCardNames, ScryfallObject, ScryfallObjectBack};
use serde_json::from_reader;
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

    let default_cards: Vec<serde_json::Map<String, serde_json::Value>> = from_reader(f).unwrap();
    println!(
        "there are {} entries in {}",
        default_cards.len(),
        &opts.input
    );

    let mut card_data_without_meld_results: std::collections::HashMap<String, Vec<ScryfallObject>> =
        std::collections::HashMap::new();
    for default_card in default_cards.iter() {
        let scryfall_object = ScryfallObject::from_dict(default_card);
        match scryfall_object {
            Some(scryfall_object) => {
                card_data_without_meld_results
                    .entry(scryfall_object.name.clone())
                    .or_default()
                    .push(scryfall_object);
            }
            None => {
                print!("couldn't convert scryfall object {:?}", default_card);
            }
        }
    }

    let mut card_data: std::collections::HashMap<String, Vec<CardPrinting>> =
        std::collections::HashMap::new();
    for (name, scryfall_objects) in card_data_without_meld_results.iter() {
        for scryfall_object in scryfall_objects {
            let back = match &scryfall_object.border_crop_back {
                Some(ScryfallObjectBack::MeldResultName(meld_name)) => {
                    let relateds = card_data_without_meld_results.get(meld_name);
                    match relateds {
                        Some(relateds) => {
                            let matching_set =
                                relateds.iter().find(|x| x.set == scryfall_object.set);
                            match matching_set {
                                Some(matching_set) => Some(matching_set.border_crop.clone()),
                                None => {
                                    print!(
                                        "related card {} with set {} not found",
                                        meld_name, scryfall_object.set
                                    );
                                    None
                                }
                            }
                        }
                        None => {
                            print!("couldn't find meld result {}", meld_name);
                            None
                        }
                    }
                }
                Some(ScryfallObjectBack::Uri(uri)) => Some(uri).cloned(),
                None => None,
            };
            let printings = card_data.entry(name.clone()).or_default();
            printings.push(CardPrinting {
                set: scryfall_object.set.clone(),
                border_crop: scryfall_object.border_crop.clone(),
                border_crop_back: back,
            });
        }
    }

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

    let outputfile = File::create(&opts.output).unwrap();
    serde_json::to_writer(outputfile, &card_data).unwrap();
}
