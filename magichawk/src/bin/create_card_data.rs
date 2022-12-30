use clap::Parser;
use magichawk::{CardPrintings, MinimalScryfallObject, ScryfallCardNames};
use serde_json::from_reader;
use std::fs::File;
use std::io::BufReader;

/// Process a bulk "Default Cards" file from the Scryfall API into a card data file for magichawk,
/// see https://scryfall.com/docs/api/bulk-data for the bulk data.
/// The bulk data list of different card names from the Scryfall API is used to exclude tokens.
#[derive(Parser, Debug)]
struct Opts {
    /// bulk "Default Cards" input filename
    input: String,
    /// output filename
    output: String,
    /// file for problem cases
    unconverted: String,
    /// file for objects which aren't cards
    notcards: String,
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
    let reader = BufReader::new(f);

    let default_cards: Vec<serde_json::Map<String, serde_json::Value>> =
        from_reader(reader).unwrap();
    println!(
        "there are {} entries in {}\n",
        default_cards.len(),
        &opts.input
    );

    let mut unconverted = Vec::new();
    let mut not_cards = Vec::new();

    let mut card_data = CardPrintings::new();
    for default_card in default_cards.iter() {
        let scryfall_object = MinimalScryfallObject::from_dict(default_card);
        match scryfall_object {
            Some(scryfall_object) => {
                if nontoken_names.names.contains(&scryfall_object.name) {
                    let language = scryfall_object.language.clone();
                    card_data
                        .printings
                        .entry(scryfall_object.name.clone())
                        .or_default()
                        .push(scryfall_object);
                    card_data.languages.insert(language);
                } else {
                    not_cards.push(default_card);
                }
            }
            None => {
                unconverted.push(default_card);
            }
        }
    }

    let different_cards: usize = card_data
        .printings
        .values()
        .map(|printings| printings.len())
        .sum();
    println!(
        "there are {} card names and {} (card name, set) combinations ({} languages) in {}\n",
        card_data.printings.len(),
        different_cards,
        card_data.languages.len(),
        &opts.output
    );

    let outputfile = File::create(&opts.output).unwrap();
    serde_json::to_writer(outputfile, &card_data).unwrap();

    println!(
        "there are {} objects which couldn't be converted",
        unconverted.len()
    );

    let unconverted_with_image: Vec<&serde_json::Map<String, serde_json::Value>> = unconverted
        .into_iter()
        .filter(|e| e["image_status"] != "missing")
        .collect();

    println!(
        "there are {} objects which couldn't be converted which have image data, saved in {}",
        unconverted_with_image.len(),
        &opts.unconverted
    );

    {
        let problemfile = File::create(&opts.unconverted).unwrap();
        serde_json::to_writer(problemfile, &unconverted_with_image).unwrap();
    }
    {
        let notcardsfile = File::create(&opts.notcards).unwrap();
        serde_json::to_writer(notcardsfile, &not_cards).unwrap();
    }
}
