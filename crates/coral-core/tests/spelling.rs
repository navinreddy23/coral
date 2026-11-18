//! A commit message checked against each dictionary: what is flagged, where, and what is
//! offered instead.

use coral_core::spelling::{self, Dialect, MessageSpelling, Misspelling};
use coral_core::wordlist::WordList;

fn flagged(dialect: Dialect, summary: &str, body: &str) -> (Vec<String>, Vec<String>) {
    let found = spelling::check(dialect, summary, body, &WordList::default()).unwrap();
    let words = |m: &[Misspelling]| m.iter().map(|m| m.word.clone()).collect();
    (words(&found.summary), words(&found.body))
}

#[test]
fn each_dialect_flags_the_others_spelling() {
    let (us, _) = flagged(Dialect::American, "Pick a colour and a color", "");
    assert_eq!(us, ["colour"]);
    let (gb, _) = flagged(Dialect::British, "Pick a colour and a color", "");
    assert_eq!(gb, ["color"]);
    let (us, _) = flagged(Dialect::American, "Optimize, then optimise", "");
    assert_eq!(us, ["optimise"]);
    let (gb, _) = flagged(Dialect::British, "Optimize, then optimise", "");
    assert_eq!(gb, ["Optimize"]);
}

#[test]
fn says_where_each_misspelling_is_in_its_own_field() {
    let found = spelling::check(
        Dialect::American,
        "core: recieve teh watcher",
        "First line.\nA seperate line.",
        &WordList::default(),
    )
    .unwrap();
    let mark = |word: &str, start, end| Misspelling {
        word: word.to_owned(),
        start,
        end,
    };
    assert_eq!(
        found,
        MessageSpelling {
            summary: vec![mark("recieve", 6, 13), mark("teh", 14, 17)],
            body: vec![mark("seperate", 14, 22)],
        }
    );
}

#[test]
fn suggests_the_word_that_was_meant_first() {
    let suggestions = spelling::suggest(Dialect::American, "teh").unwrap();
    assert!(suggestions.contains(&"the".to_owned()), "{suggestions:?}");
    let suggestions = spelling::suggest(Dialect::American, "recieve").unwrap();
    assert_eq!(suggestions.first().map(String::as_str), Some("receive"));
    // The capital of a word at the start of a sentence is kept.
    let suggestions = spelling::suggest(Dialect::American, "Teh").unwrap();
    assert!(suggestions.contains(&"The".to_owned()), "{suggestions:?}");
    let suggestions = spelling::suggest(Dialect::British, "colur").unwrap();
    assert!(
        suggestions.contains(&"colour".to_owned()),
        "{suggestions:?}"
    );
    assert!(
        spelling::suggest(Dialect::American, "seperate")
            .unwrap()
            .len()
            <= 5
    );
    // Never a split into a word and a scrap of one.
    assert_eq!(
        spelling::suggest(Dialect::American, "untill").unwrap()[0],
        "until"
    );
    for word in ["untill", "Untill", "occured", "Wether", "colour"] {
        let suggestions = spelling::suggest(Dialect::American, word).unwrap();
        assert!(
            suggestions.iter().all(|s| !s.contains([' ', '-'])),
            "{word}: {suggestions:?}"
        );
    }
}

#[test]
fn knows_the_words_someone_added() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("words.txt");
    let (before, _) = flagged(Dialect::American, "Frobnicate the frobnicate's", "");
    assert_eq!(before, ["Frobnicate", "frobnicate's"]);
    WordList::add(&path, "frobnicate").unwrap();
    let words = WordList::load(&path).unwrap();
    let found = spelling::check(Dialect::American, "Frobnicate the frobnicate's", "", &words);
    assert_eq!(found.unwrap().summary, []);
}

#[test]
fn knows_git_and_its_tools_without_being_told() {
    let (summary, body) = flagged(
        Dialect::American,
        "Rebase onto upstream and run rerere",
        "The worktree's submodules were unstaged by the autostash; reflog and fixup commits \
         show it. Tokio, serde and clippy agree.",
    );
    assert_eq!(summary, Vec::<String>::new());
    assert_eq!(body, Vec::<String>::new());
}

#[test]
fn leaves_code_paths_names_and_trailers_alone() {
    let body = "\
Call `frobnicate_all()` from src/watchr.rs before RowStore is built; see
https://example.invalid/issue/12 and #34 for @gracie.

    $ coral spel --dialect en-US
    teh output is pasted

```
let recieve = 1;
```

Signed-off-by: Ada Lovelace <ada@example.invalid>
Co-authored-by: Grace Hopper <grace@example.invalid>
";
    let (summary, body) = flagged(Dialect::American, "feat(ui)!: Merge branch 'topik'", body);
    assert_eq!(summary, Vec::<String>::new());
    assert_eq!(body, Vec::<String>::new());
}

#[test]
fn checks_a_whole_realistic_message() {
    let summary = "watcher: coalesce bursts of events behind one debounce";
    let body = "\
A kernel build writes tens of thousands of files a second, and every one of them
raised its own refresh. They are now coalesced behind a 300 ms trailing debounce,
so the window refreshes once when the build is quiet, which is what the user would
expect when they come back to it. Teh previous behaviour was to refresh on every
event, which made the window unresponsive untill the build finished.

Signed-off-by: Ada Lovelace <ada@example.invalid>
";
    let (summary, body) = flagged(Dialect::American, summary, body);
    assert_eq!(summary, Vec::<String>::new());
    assert_eq!(body, ["Teh", "behaviour", "untill"]);
    let (_, body) = flagged(Dialect::British, "", body_of_british_message());
    assert_eq!(body, ["untill"]);
}

fn body_of_british_message() -> &'static str {
    "The previous behaviour was to refresh on every event, untill the build finished."
}

#[test]
fn an_empty_message_has_nothing_to_flag() {
    let found = spelling::check(Dialect::British, "", "", &WordList::default()).unwrap();
    assert_eq!(found, MessageSpelling::default());
}
