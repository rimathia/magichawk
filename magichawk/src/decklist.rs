extern crate regex;

use lazy_static::lazy_static;
use regex::{Match, Regex};

#[derive(Debug, PartialEq, Clone)]
pub struct DecklistEntry {
    pub multiple: i32,
    pub name: String,
    pub set: Option<String>,
}

impl DecklistEntry {
    pub fn new(m: i32, n: &str, s: Option<&str>) -> DecklistEntry {
        DecklistEntry {
            multiple: m,
            name: n.to_string(),
            set: s.map(|x| x.to_string()),
        }
    }

    pub fn from_name(n: &str) -> DecklistEntry {
        DecklistEntry {
            multiple: 1,
            name: n.to_string(),
            set: None,
        }
    }

    pub fn from_multiple_name(m: i32, n: &str) -> DecklistEntry {
        DecklistEntry {
            multiple: m,
            name: n.to_string(),
            set: None,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct ParsedDecklistLine<'a> {
    line: &'a str,
    entry: Option<DecklistEntry>,
}

impl<'a> ParsedDecklistLine<'a> {
    pub fn as_entry(&self) -> Option<DecklistEntry> {
        self.entry.clone()
    }
}

fn parse_set(group: Option<Match>) -> Option<&str> {
    Some(group?.as_str())
}

fn parse_multiple(group: Option<Match>) -> i32 {
    match group {
        Some(m) => m.as_str().parse().ok().unwrap_or(1),
        None => 1,
    }
}

pub fn parse_line(line: &str) -> Option<DecklistEntry> {
    lazy_static! {
        static ref REMNS: Regex =
            Regex::new(r"^\s*(\d*)\s*([^\(\[\$\t]*)[\s\(\[]*([\dA-Za-z]{3})?").unwrap();
    }

    match REMNS.captures(line) {
        Some(mns) => {
            let multiple = parse_multiple(mns.get(1));
            let name = mns.get(2)?.as_str().trim().to_string();
            let set = parse_set(mns.get(3)).map(|s| s.to_string());
            let name_lowercase = name.to_lowercase();
            let non_entries = vec!["deck", "decklist", "sideboard"];
            if non_entries.iter().any(|s| **s == name_lowercase) {
                None
            } else {
                Some(DecklistEntry {
                    multiple,
                    name,
                    set,
                })
            }
        }
        None => None,
    }
}

pub fn parse_decklist(decklist: &str) -> Vec<ParsedDecklistLine> {
    decklist
        .lines()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| ParsedDecklistLine {
            line: s,
            entry: parse_line(s),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn name() {
        assert_eq!(
            parse_line("plains").unwrap(),
            DecklistEntry::from_name("plains")
        );
    }

    #[test]
    fn number_name() {
        assert_eq!(
            parse_line("2\tplains").unwrap(),
            DecklistEntry::from_multiple_name(2, "plains")
        );
    }

    #[test]
    fn shatter() {
        assert_eq!(
            parse_line("1 shatter [mrd]").unwrap(),
            DecklistEntry::new(1, "shatter", Some("mrd"))
        );
    }

    #[test]
    fn number_name_set() {
        assert_eq!(
            parse_line("17 long card's name [IPA]").unwrap(),
            DecklistEntry::new(17, "long card's name", Some("IPA"))
        );
    }

    #[test]
    fn name_set() {
        assert_eq!(
            parse_line("long card's name [IPA]").unwrap(),
            DecklistEntry::new(1, "long card's name", Some("IPA"))
        );
    }

    #[test]
    fn name_with_tab() {
        assert_eq!(
            parse_line("Incubation/Incongruity   \t\t---").unwrap(),
            DecklistEntry::from_multiple_name(1, "Incubation/Incongruity")
        );
    }

    #[test]
    fn mtgdecks() {
        let decklist = "4  Beanstalk Giant   		$0.25
        4  Lovestruck Beast   		$1.5
        Artifact [5]
        1  The Great Henge   		$25
        Instant [1]
        1  Incubation/Incongruity   		--- ";
        let parsed = parse_decklist(decklist);
        let expected = vec![
            ParsedDecklistLine {
                line: "4  Beanstalk Giant   		$0.25",
                entry: Some(DecklistEntry::from_multiple_name(4, "Beanstalk Giant")),
            },
            ParsedDecklistLine {
                line: "4  Lovestruck Beast   		$1.5",
                entry: Some(DecklistEntry::from_multiple_name(4, "Lovestruck Beast")),
            },
            ParsedDecklistLine {
                line: "Artifact [5]",
                entry: Some(DecklistEntry::from_multiple_name(1, "Artifact")),
            },
            ParsedDecklistLine {
                line: "1  The Great Henge   		$25",
                entry: Some(DecklistEntry::from_multiple_name(1, "The Great Henge")),
            },
            ParsedDecklistLine {
                line: "Instant [1]",
                entry: Some(DecklistEntry::from_multiple_name(1, "Instant")),
            },
            ParsedDecklistLine {
                line: "1  Incubation/Incongruity   		---",
                entry: Some(DecklistEntry::from_multiple_name(
                    1,
                    "Incubation/Incongruity",
                )),
            },
        ];
        for (left, right) in parsed.iter().zip(expected.iter()) {
            assert_eq!(left, right);
        }
    }

    #[test]
    fn arenaexport() {
        let decklist = "Deck
        1 Bedeck // Bedazzle (RNA) 221
        1 Spawn of Mayhem (RNA) 85
        ";
        let expected = vec![
            ParsedDecklistLine {
                line: "Deck",
                entry: None,
            },
            ParsedDecklistLine {
                line: "1 Bedeck // Bedazzle (RNA) 221",
                entry: Some(DecklistEntry::new(1, "Bedeck // Bedazzle", Some("RNA"))),
            },
            ParsedDecklistLine {
                line: "1 Spawn of Mayhem (RNA) 85",
                entry: Some(DecklistEntry::new(1, "Spawn of Mayhem", Some("RNA"))),
            },
        ];
        let parsed = parse_decklist(decklist);
        for (left, right) in parsed.iter().zip(expected.iter()) {
            assert_eq!(left, right);
        }
    }

    #[test]
    fn arenaexport2() {
        let decklist = "Deck\n1 Defiant Strike (M21) 15\n24 Plains (ANB) 115\n\nSideboard\n2 Faerie Guidemother (ELD) 11";
        let expected = vec![
            ParsedDecklistLine {
                line: "Deck",
                entry: None,
            },
            ParsedDecklistLine {
                line: "1 Defiant Strike (M21) 15",
                entry: Some(DecklistEntry::new(1, "Defiant Strike", Some("M21"))),
            },
            ParsedDecklistLine {
                line: "24 Plains (ANB) 115",
                entry: Some(DecklistEntry::new(24, "Plains", Some("ANB"))),
            },
            ParsedDecklistLine {
                line: "Sideboard",
                entry: None,
            },
            ParsedDecklistLine {
                line: "2 Faerie Guidemother (ELD) 11",
                entry: Some(DecklistEntry::new(2, "Faerie Guidemother", Some("ELD"))),
            },
        ];
        let parsed = parse_decklist(decklist);
        for (left, right) in parsed.iter().zip(expected.iter()) {
            assert_eq!(left, right);
        }

        // not necessary anymore because we filter out the lines "deck" and "sideboard" manually now
        // let mut card_data = CardData::from_bulk(
        //     serde_json::from_reader(
        //         //serde_json::from_reader::<HashMap<String, Vec<CardPrinting>>(
        //         std::fs::File::open("assets/card_data.json").unwrap(),
        //     )
        //     .unwrap(),
        // )
        // .unwrap();

        // let imagelines = image_lines_from_decklist(parsed, &mut card_data, BacksideMode::One);
        // assert_eq!(imagelines.len(), 3);
    }
}
