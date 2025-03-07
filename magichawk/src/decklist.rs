extern crate regex;

use lazy_static::lazy_static;
use regex::{Match, Regex};
use std::collections::HashSet;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DecklistEntry {
    pub multiple: i32,
    pub name: String,
    pub set: Option<String>,
    pub lang: Option<String>,
}

impl DecklistEntry {
    pub fn new(multiple: i32, name: &str, set: Option<&str>, lang: Option<&str>) -> DecklistEntry {
        DecklistEntry {
            multiple,
            name: name.to_string(),
            set: set.map(String::from),
            lang: lang.map(String::from),
        }
    }

    pub fn from_name(n: &str) -> DecklistEntry {
        DecklistEntry {
            multiple: 1,
            name: n.to_string(),
            set: None,
            lang: None,
        }
    }

    pub fn from_multiple_name(m: i32, n: &str) -> DecklistEntry {
        DecklistEntry {
            multiple: m,
            name: n.to_string(),
            set: None,
            lang: None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParsedDecklistLine<'a> {
    line: &'a str,
    entry: Option<DecklistEntry>,
}

impl ParsedDecklistLine<'_> {
    pub fn as_entry(&self) -> Option<DecklistEntry> {
        self.entry.clone()
    }
}

fn parse_multiple(group: Option<Match>) -> i32 {
    match group {
        Some(m) => m.as_str().parse().ok().unwrap_or(1),
        None => 1,
    }
}

fn parse_set(group: Option<Match>) -> Option<String> {
    Some(group?.as_str().parse::<String>().ok()?.to_lowercase())
}
fn parse_lang(group: Option<Match>, languages: &HashSet<String>) -> Option<String> {
    let lang = group?.as_str().parse::<String>().ok()?.to_lowercase();
    if languages.contains(&lang) {
        Some(lang)
    } else {
        None
    }
}

pub fn parse_line(line: &str, languages: &HashSet<String>) -> Option<DecklistEntry> {
    lazy_static! {
        static ref REMNS: Regex =
            Regex::new(r"^\s*(\d*)\s*([^\(\[\$\t]*)[\s\(\[]*([\dA-Za-z]{2,3})?").unwrap();
    }

    match REMNS.captures(line) {
        Some(mns) => {
            let multiple = parse_multiple(mns.get(1));
            let name = mns.get(2)?.as_str().trim().to_string();
            let set_or_lang = mns.get(3);
            let set = parse_set(set_or_lang);
            let lang = parse_lang(set_or_lang, languages);
            let name_lowercase = name.to_lowercase();
            let non_entries = ["deck", "decklist", "sideboard"];
            if non_entries.iter().any(|s| **s == name_lowercase) {
                None
            } else {
                Some(DecklistEntry {
                    multiple,
                    name,
                    set,
                    lang,
                })
            }
        }
        None => None,
    }
}

pub fn parse_decklist<'a>(
    decklist: &'a str,
    languages: &HashSet<String>,
) -> Vec<ParsedDecklistLine<'a>> {
    decklist
        .lines()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| ParsedDecklistLine {
            line: s,
            entry: parse_line(s, languages),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::scryfall::get_minimal_scryfall_languages;

    fn parse_line_default(s: &str) -> Option<DecklistEntry> {
        let minimal = get_minimal_scryfall_languages();
        parse_line(s, &minimal)
    }

    fn parse_decklist_default(s: &str) -> Vec<ParsedDecklistLine> {
        let minimal = get_minimal_scryfall_languages();
        parse_decklist(s, &minimal)
    }

    use super::*;
    #[test]
    fn name() {
        assert_eq!(
            parse_line_default("plains").unwrap(),
            DecklistEntry::from_name("plains")
        );
    }

    #[test]
    fn number_name() {
        assert_eq!(
            parse_line_default("2\tplains").unwrap(),
            DecklistEntry::from_multiple_name(2, "plains")
        );
    }

    #[test]
    fn shatter() {
        assert_eq!(
            parse_line_default("1 shatter [mrd]").unwrap(),
            DecklistEntry::new(1, "shatter", Some("mrd"), None)
        );
    }

    #[test]
    fn number_name_set() {
        assert_eq!(
            parse_line_default("17 long card's name [IPA]").unwrap(),
            DecklistEntry::new(17, "long card's name", Some("ipa"), None)
        );
    }

    #[test]
    fn name_set() {
        assert_eq!(
            parse_line_default("long card's name [ipa]").unwrap(),
            DecklistEntry::new(1, "long card's name", Some("ipa"), None)
        );
    }

    #[test]
    fn name_with_tab() {
        assert_eq!(
            parse_line_default("Incubation/Incongruity   \t\t---").unwrap(),
            DecklistEntry::from_multiple_name(1, "Incubation/Incongruity")
        );
    }

    #[test]
    fn japanese_printing() {
        assert_eq!(
            parse_line_default("memory lapse [ja]").unwrap(),
            DecklistEntry::new(1, "memory lapse", Some("ja"), Some("ja"))
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
        let parsed = parse_decklist_default(decklist);
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
                entry: Some(DecklistEntry::new(
                    1,
                    "Bedeck // Bedazzle",
                    Some("rna"),
                    None,
                )),
            },
            ParsedDecklistLine {
                line: "1 Spawn of Mayhem (RNA) 85",
                entry: Some(DecklistEntry::new(1, "Spawn of Mayhem", Some("rna"), None)),
            },
        ];
        let parsed = parse_decklist_default(decklist);
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
                entry: Some(DecklistEntry::new(1, "Defiant Strike", Some("m21"), None)),
            },
            ParsedDecklistLine {
                line: "24 Plains (ANB) 115",
                entry: Some(DecklistEntry::new(24, "Plains", Some("anb"), None)),
            },
            ParsedDecklistLine {
                line: "Sideboard",
                entry: None,
            },
            ParsedDecklistLine {
                line: "2 Faerie Guidemother (ELD) 11",
                entry: Some(DecklistEntry::new(
                    2,
                    "Faerie Guidemother",
                    Some("eld"),
                    None,
                )),
            },
        ];
        let parsed = parse_decklist_default(decklist);
        for (left, right) in parsed.iter().zip(expected.iter()) {
            assert_eq!(left, right);
        }
    }
}
