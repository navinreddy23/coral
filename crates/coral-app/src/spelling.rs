//! The commit box's spelling check. The dictionary is read the first time a message is checked;
//! the word list on every check, so a word added from the command line counts straight away.

use coral_core::CoralError;
use coral_core::spelling::{self, Dialect, MessageSpelling};
use coral_core::wordlist::{self, WordList};

use crate::commands::IpcError;

/// The misspellings of the summary and of the body, each counted in UTF-16 units from its own
/// start, as the text box holding it counts.
///
/// # Errors
/// [`CoralError::Io`] when the word list cannot be read.
#[tauri::command]
pub async fn spelling_check(
    summary: String,
    body: String,
    dialect: Dialect,
) -> Result<MessageSpelling, IpcError> {
    blocking("check spelling", move || {
        let words =
            wordlist::file().map_or_else(|| Ok(WordList::default()), |p| WordList::load(&p))?;
        spelling::check(dialect, &summary, &body, &words)
    })
    .await
}

/// What a misspelt word might have meant, likeliest first and at most five.
///
/// # Errors
/// Only if the built-in dictionary cannot be read, which a test rules out.
#[tauri::command]
pub async fn spelling_suggest(word: String, dialect: Dialect) -> Result<Vec<String>, IpcError> {
    blocking("suggest a spelling", move || {
        spelling::suggest(dialect, &word)
    })
    .await
}

/// Adds a word to the list the window and `coral spell` both accept.
///
/// # Errors
/// [`CoralError::Refused`] for a word with a space in it; [`CoralError::Io`] when the list
/// cannot be written.
#[tauri::command]
pub async fn spelling_learn(word: String) -> Result<(), IpcError> {
    blocking("add the word", move || {
        let list = wordlist::file().ok_or_else(|| CoralError::Refused {
            label: "add the word",
            detail: "there is no config directory to keep the list in".to_owned(),
        })?;
        WordList::add(&list, &word)
    })
    .await
}

// Off the async runtime: the first check reads a dictionary of fifty thousand words, and a
// suggestion is a search of it.
async fn blocking<T: Send + 'static>(
    label: &'static str,
    work: impl FnOnce() -> Result<T, CoralError> + Send + 'static,
) -> Result<T, IpcError> {
    let done = tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| CoralError::Protocol {
            label,
            detail: e.to_string(),
        })?;
    Ok(done?)
}

#[cfg(test)]
mod tests {
    use coral_core::spelling::Dialect;

    use super::{spelling_check, spelling_suggest};

    #[tokio::test]
    async fn checks_each_field_and_counts_as_the_text_box_does() {
        let found = spelling_check(
            "🎉 Fix teh watcher".to_owned(),
            "It recieves\nevents seperately.".to_owned(),
            Dialect::American,
        )
        .await
        .unwrap();
        let summary: Vec<_> = found
            .summary
            .iter()
            .map(|m| (m.word.as_str(), m.start, m.end))
            .collect();
        let body: Vec<_> = found
            .body
            .iter()
            .map(|m| (m.word.as_str(), m.start, m.end))
            .collect();
        assert_eq!(summary, [("teh", 7, 10)]);
        assert_eq!(body, [("recieves", 3, 11), ("seperately", 19, 29)]);
    }

    #[tokio::test]
    async fn suggests_from_the_dialect_asked_for() {
        let american = spelling_suggest("colur".to_owned(), Dialect::American)
            .await
            .unwrap();
        let british = spelling_suggest("colur".to_owned(), Dialect::British)
            .await
            .unwrap();
        assert_eq!(american.first().map(String::as_str), Some("color"));
        assert_eq!(british.first().map(String::as_str), Some("colour"));
    }
}
