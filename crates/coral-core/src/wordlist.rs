//! The words a person has told the spelling check are right, kept beside the window's settings
//! so the window and `coral spell` agree on them.

use std::collections::HashSet;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::error::CoralError;

/// Where the list is kept. Tauri's config directory is `dirs::config_dir()` joined with the
/// bundle identifier, so the window's own settings are in the same place and the CLI, which
/// has no Tauri, finds the list without asking it.
#[must_use]
pub fn file() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("dev.coral.app").join("words.txt"))
}

/// A set of known words, matched the way a Hunspell dictionary matches its own: an entry in
/// lowercase is also known capitalised or in capitals, and one with capitals of its own only
/// as written or in capitals.
#[derive(Debug, Default)]
pub struct WordList {
    words: HashSet<String>,
    upper: HashSet<String>,
}

impl WordList {
    /// The list at `path`, which is empty until a word has been added.
    ///
    /// # Errors
    /// [`CoralError::Io`] when the file exists and cannot be read.
    pub fn load(path: &Path) -> Result<Self, CoralError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(Self::parse(&text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// One word to a line; blank lines and the space around a word are ignored.
    pub(crate) fn parse(text: &str) -> Self {
        let words: HashSet<String> = text
            .lines()
            .map(str::trim)
            .filter(|w| !w.is_empty())
            .map(normalised)
            .collect();
        let upper = words.iter().map(|w| w.to_uppercase()).collect();
        Self { words, upper }
    }

    /// Whether `word` is on the list, as written or as its possessive.
    #[must_use]
    pub fn contains(&self, word: &str) -> bool {
        let word = normalised(word);
        self.knows(&word) || word.strip_suffix("'s").is_some_and(|stem| self.knows(stem))
    }

    fn knows(&self, word: &str) -> bool {
        if self.words.contains(word) {
            return true;
        }
        let upper = is_upper(word);
        ((upper || is_title(word)) && self.words.contains(&word.to_lowercase()))
            || (upper && self.upper.contains(word))
    }

    /// Adds `word` to the list at `path`, which is kept sorted and without repeats, and made
    /// with its directory when it does not exist yet.
    ///
    /// # Errors
    /// [`CoralError::Refused`] for an empty word or one with a space in it;
    /// [`CoralError::Io`] when the list cannot be read or written.
    pub fn add(path: &Path, word: &str) -> Result<(), CoralError> {
        let word = word.trim();
        if word.is_empty() || word.contains(char::is_whitespace) {
            return Err(CoralError::Refused {
                label: "add the word",
                detail: "a word to add is one word, with no spaces in it".to_owned(),
            });
        }
        let existing = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e.into()),
        };
        let mut words: Vec<String> = existing
            .lines()
            .map(str::trim)
            .filter(|w| !w.is_empty())
            .map(normalised)
            .collect();
        words.push(normalised(word));
        words.sort();
        words.dedup();
        write(path, &(words.join("\n") + "\n"))
    }
}

/// Writes through a file beside the target and renames it into place, so a list being written
/// by the window while `coral spell --add` writes it too is never read half-written.
fn write(path: &Path, text: &str) -> Result<(), CoralError> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let mut temp = tempfile::NamedTempFile::new_in(dir)?;
    temp.write_all(text.as_bytes())?;
    temp.persist(path).map_err(|e| CoralError::Io(e.error))?;
    Ok(())
}

/// The typographic apostrophe a phone or a word processor puts in, as the ASCII one the
/// dictionaries are written with.
pub(crate) fn normalised(word: &str) -> String {
    word.replace('\u{2019}', "'")
}

fn is_upper(word: &str) -> bool {
    word.chars().any(char::is_alphabetic) && !word.chars().any(char::is_lowercase)
}

fn is_title(word: &str) -> bool {
    let mut chars = word.chars();
    chars.next().is_some_and(char::is_uppercase) && !chars.any(char::is_uppercase)
}

#[cfg(test)]
mod tests {
    use super::WordList;

    #[test]
    fn a_lowercase_entry_is_known_capitalised_and_in_capitals() {
        let list = WordList::parse("gix\n");
        assert!(list.contains("gix"));
        assert!(list.contains("Gix"));
        assert!(list.contains("GIX"));
        assert!(!list.contains("gIx"));
        assert!(!list.contains("gixx"));
    }

    #[test]
    fn an_entry_with_capitals_is_known_only_as_written_or_in_capitals() {
        let list = WordList::parse("GitHub\n");
        assert!(list.contains("GitHub"));
        assert!(list.contains("GITHUB"));
        assert!(!list.contains("github"));
        assert!(!list.contains("Github"));
    }

    #[test]
    fn knows_the_possessive_and_either_apostrophe() {
        let list = WordList::parse("tokio\ndon't\n");
        assert!(list.contains("tokio's"));
        assert!(list.contains("Tokio\u{2019}s"));
        assert!(list.contains("don\u{2019}t"));
        assert!(!list.contains("tokios"));
    }

    #[test]
    fn ignores_blank_lines_and_the_space_around_a_word() {
        let list = WordList::parse("\n  rerere  \n\r\n\tfixup\n");
        assert!(list.contains("rerere"));
        assert!(list.contains("fixup"));
        assert!(!list.contains(""));
    }
}
