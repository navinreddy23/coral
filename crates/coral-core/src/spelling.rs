//! Spelling in a commit message, against US or British English.
//!
//! The dictionaries are SCOWL's Hunspell ones, built in, so a message is checked the same way
//! on every platform and by the window and `coral spell` alike. Each is read the first time it
//! is asked for, once per process.

use std::collections::HashSet;
use std::str::FromStr;
use std::sync::{LazyLock, OnceLock};

use spellbook::Dictionary;

use crate::error::CoralError;
use crate::prose::{self, Word};
use crate::wordlist::{self, WordList};

// No dictionary offers more than this many that are worth reading in a menu.
const SUGGESTIONS: usize = 5;

/// Which English a message is checked against.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
pub enum Dialect {
    #[serde(rename = "en-US")]
    American,
    #[serde(rename = "en-GB")]
    British,
}

/// A word the dictionary does not know, where it is in the text it was found in. Offsets count
/// UTF-16 code units, which is how a text box counts.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
pub struct Misspelling {
    pub word: String,
    pub start: u32,
    pub end: u32,
}

/// The misspellings of a message's summary and of its body, each counted from its own start.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
pub struct MessageSpelling {
    pub summary: Vec<Misspelling>,
    pub body: Vec<Misspelling>,
}

static AMERICAN: OnceLock<Result<Dictionary, String>> = OnceLock::new();
static BRITISH: OnceLock<Result<Dictionary, String>> = OnceLock::new();

// Words of git and of the tools around it, which a general dictionary has not heard of and a
// commit message uses all the time.
static SOFTWARE: LazyLock<WordList> =
    LazyLock::new(|| WordList::parse(include_str!("../dictionaries/software.txt")));

impl Dialect {
    /// The tag it is known by, as in `en-US`.
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Self::American => "en-US",
            Self::British => "en-GB",
        }
    }

    fn dictionary(self) -> Result<&'static Dictionary, CoralError> {
        let (cell, aff, dic) = match self {
            Self::American => (
                &AMERICAN,
                include_str!("../dictionaries/en_US.aff"),
                include_str!("../dictionaries/en_US.dic"),
            ),
            Self::British => (
                &BRITISH,
                include_str!("../dictionaries/en_GB-ise.aff"),
                include_str!("../dictionaries/en_GB-ise.dic"),
            ),
        };
        cell.get_or_init(|| Dictionary::new(aff, dic).map_err(|e| e.to_string()))
            .as_ref()
            .map_err(|e| CoralError::Refused {
                label: "check spelling",
                detail: format!("the {} dictionary could not be read: {e}", self.tag()),
            })
    }
}

impl FromStr for Dialect {
    type Err = CoralError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().replace('_', "-").as_str() {
            "en-us" => Ok(Self::American),
            "en-gb" => Ok(Self::British),
            _ => Err(CoralError::Refused {
                label: "check spelling",
                detail: format!("there is no dictionary for {s}; there is en-US and en-GB"),
            }),
        }
    }
}

/// The words of a message's summary and body that neither the dictionary, git's own
/// vocabulary nor `words` knows.
///
/// # Errors
/// [`CoralError::Refused`] if the built-in dictionary cannot be read, which a test rules out.
pub fn check(
    dialect: Dialect,
    summary: &str,
    body: &str,
    words: &WordList,
) -> Result<MessageSpelling, CoralError> {
    let dictionary = dialect.dictionary()?;
    let unknown = |w: &Word<'_>| !known(dictionary, words, w.text);
    Ok(MessageSpelling {
        summary: located(summary, prose::summary(summary).into_iter().filter(unknown)),
        body: located(body, prose::body(body).into_iter().filter(unknown)),
    })
}

/// What `word` might have been meant to be, likeliest first.
///
/// # Errors
/// [`CoralError::Refused`] if the built-in dictionary cannot be read.
pub fn suggest(dialect: Dialect, word: &str) -> Result<Vec<String>, CoralError> {
    let dictionary = dialect.dictionary()?;
    let word = wordlist::normalised(word);
    let mut found = Vec::new();
    // Hunspell's capitalised lookup misses what the word in lowercase finds: "Teh" was offered
    // "Eh" and "Th" but never "The". The capital is put back on what the lowercase finds.
    if let Some(lower) = lowered_title(&word) {
        found.extend(
            suggestions(dictionary, &lower)
                .iter()
                .map(|s| capitalised(s)),
        );
    }
    found.extend(suggestions(dictionary, &word));
    let mut seen = HashSet::new();
    found.retain(|s| !is_torn(s, &word) && seen.insert(s.clone()));
    found.truncate(SUGGESTIONS);
    Ok(found)
}

fn suggestions(dictionary: &Dictionary, word: &str) -> Vec<String> {
    let mut out = Vec::new();
    dictionary.suggest(word, &mut out);
    out
}

fn lowered_title(word: &str) -> Option<String> {
    let mut chars = word.chars();
    let first = chars.next()?;
    (first.is_uppercase() && chars.all(|c| !c.is_uppercase())).then(|| word.to_lowercase())
}

fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// A suggestion that splits the word in two where one side is a stray letter or an ending
/// rather than a word: Hunspell offers "until l" for "untill" and "occur ed" for "occured".
/// A split into real words, "a lot" for "alot", is kept.
fn is_torn(suggestion: &str, word: &str) -> bool {
    const ENDINGS: &[&str] = &["d", "s", "ed", "es", "er", "ly", "ing"];
    let split = |c: char| c == ' ' || c == '-';
    if !suggestion.contains(split) || word.contains(split) {
        return false;
    }
    suggestion.split(split).any(|part| {
        let lower = part.to_lowercase();
        (part.chars().count() == 1 && !matches!(part, "a" | "A" | "I"))
            || ENDINGS.contains(&lower.as_str())
    })
}

fn known(dictionary: &Dictionary, words: &WordList, word: &str) -> bool {
    let word = wordlist::normalised(word);
    dictionary.check(&word)
        || SOFTWARE.contains(&word)
        || words.contains(&word)
        || unaccented(&word).is_some_and(|plain| dictionary.check(&plain))
}

/// The word without its accents, when it has any and they are all ones Latin-1 has. SCOWL's
/// dictionaries are built with accents stripped, so they know "cafe" and not "café".
fn unaccented(word: &str) -> Option<String> {
    if word.is_ascii() {
        return None;
    }
    word.chars()
        .map(|c| match c {
            'à'..='å' => Some('a'),
            'À'..='Å' => Some('A'),
            'ç' => Some('c'),
            'Ç' => Some('C'),
            'è'..='ë' => Some('e'),
            'È'..='Ë' => Some('E'),
            'ì'..='ï' => Some('i'),
            'Ì'..='Ï' => Some('I'),
            'ñ' => Some('n'),
            'Ñ' => Some('N'),
            'ò'..='ö' | 'ø' => Some('o'),
            'Ò'..='Ö' | 'Ø' => Some('O'),
            'ù'..='ü' => Some('u'),
            'Ù'..='Ü' => Some('U'),
            'ý' | 'ÿ' => Some('y'),
            'Ý' => Some('Y'),
            c if c.is_ascii() => Some(c),
            _ => None,
        })
        .collect()
}

/// Each word with its UTF-16 offsets in `text`. The words come in order, so the count carries
/// on from one to the next rather than starting over.
fn located<'a>(text: &str, words: impl Iterator<Item = Word<'a>>) -> Vec<Misspelling> {
    let mut byte = 0;
    let mut unit = 0;
    words
        .map(|w| {
            unit += text[byte..w.at].encode_utf16().count();
            let start = unit;
            unit += w.text.encode_utf16().count();
            byte = w.at + w.text.len();
            Misspelling {
                word: w.text.to_owned(),
                start: saturated(start),
                end: saturated(unit),
            }
        })
        .collect()
}

fn saturated(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::{Dialect, capitalised, check, is_torn, located, lowered_title, unaccented};
    use crate::prose;
    use crate::wordlist::WordList;

    #[test]
    fn both_dictionaries_can_be_read() {
        assert!(Dialect::American.dictionary().is_ok());
        assert!(Dialect::British.dictionary().is_ok());
    }

    #[test]
    fn counts_offsets_the_way_a_text_box_does() {
        // An emoji is two UTF-16 units and four bytes; an accented letter is one and two.
        let text = "🎉 café teh";
        let found = located(text, prose::summary(text).into_iter().skip(1));
        assert_eq!(found[0].word, "teh");
        assert_eq!((found[0].start, found[0].end), (8, 11));
        let units: Vec<u16> = text.encode_utf16().collect();
        assert_eq!(String::from_utf16_lossy(&units[8..11]), "teh");
    }

    #[test]
    fn knows_accented_spellings_of_words_it_has_without_them() {
        assert_eq!(unaccented("café").as_deref(), Some("cafe"));
        assert_eq!(unaccented("Ünïcode").as_deref(), Some("Unicode"));
        assert_eq!(unaccented("plain"), None);
        assert_eq!(unaccented("Straße"), None);
        let words = WordList::default();
        let found = check(Dialect::American, "a naïve café façade", "", &words).unwrap();
        assert_eq!(found.summary, []);
    }

    #[test]
    fn drops_a_split_into_a_word_and_a_scrap() {
        assert!(is_torn("until l", "untill"));
        assert!(is_torn("occur ed", "occured"));
        assert!(is_torn("occur-ed", "occured"));
        assert!(is_torn("W ether", "Wether"));
        assert!(!is_torn("a lot", "alot"));
        assert!(!is_torn("no one", "noone"));
        assert!(!is_torn("until", "untill"));
    }

    #[test]
    fn puts_a_capital_back_on_what_the_lowercase_finds() {
        assert_eq!(lowered_title("Teh").as_deref(), Some("teh"));
        assert_eq!(lowered_title("teh"), None);
        assert_eq!(lowered_title("TEH"), None);
        assert_eq!(capitalised("the"), "The");
        assert_eq!(capitalised("über"), "Über");
        assert_eq!(capitalised(""), "");
    }

    #[test]
    fn reads_dialect_tags_loosely_and_refuses_others() {
        assert_eq!("en-US".parse::<Dialect>().unwrap(), Dialect::American);
        assert_eq!("en_gb".parse::<Dialect>().unwrap(), Dialect::British);
        assert!("fr-FR".parse::<Dialect>().is_err());
        assert_eq!(Dialect::British.tag(), "en-GB");
    }
}
