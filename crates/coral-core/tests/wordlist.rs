//! The personal word list on disk: made on the first word, kept sorted and without repeats,
//! and never left half-written.

use coral_core::CoralError;
use coral_core::wordlist::{self, WordList};

#[test]
fn a_list_that_does_not_exist_yet_knows_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let list = WordList::load(&dir.path().join("words.txt")).unwrap();
    assert!(!list.contains("rerere"));
}

#[test]
fn adding_a_word_makes_the_list_and_its_directory() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dev.coral.app").join("words.txt");
    WordList::add(&path, "frobnicate").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "frobnicate\n");
    assert!(WordList::load(&path).unwrap().contains("Frobnicate"));
}

#[test]
fn keeps_the_list_sorted_and_without_repeats() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("words.txt");
    std::fs::write(&path, "zebra\n\n  gix \n").unwrap();
    WordList::add(&path, "rerere").unwrap();
    WordList::add(&path, "gix").unwrap();
    WordList::add(&path, " tokio ").unwrap();
    WordList::add(&path, "don\u{2019}t").unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "don't\ngix\nrerere\ntokio\nzebra\n"
    );
}

#[test]
fn refuses_an_empty_word_or_one_with_a_space() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("words.txt");
    for word in ["", "   ", "two words", "tab\there"] {
        let refused = WordList::add(&path, word);
        assert!(
            matches!(refused, Err(CoralError::Refused { .. })),
            "{word:?}"
        );
    }
    assert!(!path.exists());
}

#[test]
fn leaves_nothing_but_the_list_behind() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("words.txt");
    WordList::add(&path, "one").unwrap();
    WordList::add(&path, "two").unwrap();
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, ["words.txt"]);
}

#[test]
fn a_list_that_cannot_be_read_says_so() {
    let dir = tempfile::tempdir().unwrap();
    // A directory where the file should be.
    let path = dir.path().join("words.txt");
    std::fs::create_dir(&path).unwrap();
    assert!(matches!(WordList::load(&path), Err(CoralError::Io(_))));
}

#[test]
fn is_kept_beside_the_window_settings() {
    let file = wordlist::file().unwrap();
    assert!(
        file.ends_with("dev.coral.app/words.txt"),
        "{}",
        file.display()
    );
}
