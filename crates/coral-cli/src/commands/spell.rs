use std::fmt::Write as _;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use coral_core::CoralError;
use coral_core::spelling::{self, Dialect};
use coral_core::wordlist::WordList;
use serde::Serialize;

// git writes this line into the message file of a `commit --verbose`, with the diff under it.
const SCISSORS: &str = "# ------------------------ >8 ------------------------";

/// Where a message to check comes from.
pub enum Source {
    Text(String),
    File(PathBuf),
    Stdin,
}

#[derive(Debug, Serialize)]
pub struct Checked {
    dialect: Dialect,
    misspellings: Vec<Found>,
}

/// A misspelling by line and column, counted from one in characters, as an editor shows them.
#[derive(Debug, Serialize)]
pub struct Found {
    word: String,
    line: usize,
    column: usize,
    suggestions: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Added {
    words: Vec<String>,
    list: PathBuf,
}

/// Checks a message as the commit box does: its first line as the summary, the rest as the
/// body. A message read from a file is cleaned up as git cleans one up after the editor, so a
/// commit-msg hook can hand over `.git/COMMIT_EDITMSG` as it is.
///
/// # Errors
/// [`CoralError::Io`] when the message or the word list cannot be read.
pub fn check(source: Source, dialect: Dialect, list: Option<&Path>) -> Result<Checked, CoralError> {
    let text = match source {
        Source::Text(text) => text,
        Source::File(path) if path == Path::new("-") => uncommented(&stdin()?),
        Source::File(path) => uncommented(&std::fs::read_to_string(path)?),
        Source::Stdin => stdin()?,
    };
    let words = list.map_or_else(|| Ok(WordList::default()), WordList::load)?;
    let (summary, body) = text.split_once('\n').unwrap_or((&text, ""));
    let found = spelling::check(dialect, summary, body, &words)?;
    let mut misspellings = Vec::new();
    for (field, first_line, marks) in [(summary, 1, found.summary), (body, 2, found.body)] {
        for mark in marks {
            let (line, column) = position(field, mark.start);
            misspellings.push(Found {
                suggestions: spelling::suggest(dialect, &mark.word)?,
                word: mark.word,
                line: first_line + line,
                column,
            });
        }
    }
    Ok(Checked {
        dialect,
        misspellings,
    })
}

/// Adds each word to the list.
///
/// # Errors
/// [`CoralError::Refused`] for a word with a space in it, or when there is no list to add to;
/// [`CoralError::Io`] when the list cannot be written.
pub fn add(words: &[String], list: Option<&Path>) -> Result<Added, CoralError> {
    let list = list.ok_or_else(|| CoralError::Refused {
        label: "add the word",
        detail: "there is no config directory to keep the list in; name one with --words"
            .to_owned(),
    })?;
    for word in words {
        WordList::add(list, word)?;
    }
    Ok(Added {
        words: words.iter().map(|w| w.trim().to_owned()).collect(),
        list: list.to_owned(),
    })
}

fn stdin() -> Result<String, CoralError> {
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text)?;
    Ok(text)
}

/// The text with git's comment lines blanked and everything from the scissors line on gone.
/// Blanked rather than dropped, so a line number still points at the line in the file.
fn uncommented(text: &str) -> String {
    let end = text.find(SCISSORS).unwrap_or(text.len());
    text[..end]
        .lines()
        .map(|line| if line.starts_with('#') { "" } else { line })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The line, counted from zero, and the column, counted from one in characters, of the UTF-16
/// offset `unit` in `text`.
fn position(text: &str, unit: u32) -> (usize, usize) {
    let unit = usize::try_from(unit).unwrap_or(usize::MAX);
    let mut line = 0;
    let mut column = 1;
    let mut at = 0;
    for c in text.chars() {
        if at >= unit {
            break;
        }
        at += c.len_utf16();
        if c == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

impl crate::output::Human for Checked {
    fn human(&self) -> String {
        if self.misspellings.is_empty() {
            return format!("no misspellings ({})", self.dialect.tag());
        }
        let count = self.misspellings.len();
        let noun = if count == 1 {
            "misspelling"
        } else {
            "misspellings"
        };
        let mut out = format!("{count} {noun} ({})", self.dialect.tag());
        for found in &self.misspellings {
            let offered = if found.suggestions.is_empty() {
                "(no suggestions)".to_owned()
            } else {
                format!("\u{2192} {}", found.suggestions.join(", "))
            };
            let _ = write!(
                out,
                "\n  {}:{} {} {offered}",
                found.line, found.column, found.word
            );
        }
        out
    }
}

impl crate::output::Human for Added {
    fn human(&self) -> String {
        format!("added {} to {}", self.words.join(", "), self.list.display())
    }
}

#[cfg(test)]
mod tests {
    use super::{position, uncommented};

    #[test]
    fn counts_lines_and_characters_from_utf16_offsets() {
        assert_eq!(position("teh", 0), (0, 1));
        assert_eq!(position("a teh", 2), (0, 3));
        assert_eq!(position("one\ntwo teh", 8), (1, 5));
        // The emoji is two UTF-16 units and one character.
        assert_eq!(position("🎉 teh", 3), (0, 3));
    }

    #[test]
    fn blanks_comments_and_cuts_at_the_scissors() {
        let file = "Summary\n# a comment\nbody\n# ------------------------ >8 ------------------------\ndiff --git a/x b/x\n+teh\n";
        assert_eq!(uncommented(file), "Summary\n\nbody");
    }
}
