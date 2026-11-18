//! `coral spell`: a message checked as the commit box checks it, from an argument, a file or
//! stdin, and words added to the list it reads. Every case names its own word list, so none
//! reads or writes the one on the machine running them.

use std::ffi::OsString;
use std::io::Write as _;
use std::process::{Command, Stdio};

const MESSAGE: &str = "core: Fix teh colour of the watcher\n\nIt recieves the color too.\n";

fn argv(args: &[&str]) -> Vec<OsString> {
    std::iter::once("coral")
        .chain(args.iter().copied())
        .map(OsString::from)
        .collect()
}

fn list(dir: &tempfile::TempDir) -> String {
    dir.path().join("words.txt").to_str().unwrap().to_owned()
}

fn words(json: &serde_json::Value) -> Vec<String> {
    json["result"]["misspellings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["word"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn checks_a_message_in_american_english() {
    let dir = tempfile::tempdir().unwrap();
    let out = coral_cli::run(argv(&["--json", "spell", "--words", &list(&dir), MESSAGE])).await;
    insta::assert_json_snapshot!("spell_american", out.json);
}

#[tokio::test]
async fn checks_the_same_message_in_british_english() {
    let dir = tempfile::tempdir().unwrap();
    let words_file = list(&dir);
    let args = [
        "--json",
        "spell",
        "--dialect",
        "en-GB",
        "--words",
        &words_file,
        MESSAGE,
    ];
    let out = coral_cli::run(argv(&args)).await;
    insta::assert_json_snapshot!("spell_british", out.json);
}

#[tokio::test]
async fn reads_like_an_editor_for_a_person() {
    let dir = tempfile::tempdir().unwrap();
    let out = coral_cli::run(argv(&["spell", "--words", &list(&dir), MESSAGE])).await;
    assert_eq!(
        out.text,
        "3 misspellings (en-US)\n  1:11 teh \u{2192} the, eh, tech, tee, tea\n  1:15 colour \u{2192} color, cornflour\n  3:4 recieves \u{2192} receives, relieves, reprieves, retrieves, recessives"
    );
    let clean = coral_cli::run(argv(&["spell", "--words", &list(&dir), "Fix the watcher"])).await;
    assert_eq!(clean.text, "no misspellings (en-US)");
    assert_eq!(words(&clean.json), Vec::<String>::new());
}

#[tokio::test]
async fn a_word_added_is_known_from_then_on() {
    let dir = tempfile::tempdir().unwrap();
    let words_file = list(&dir);
    let before = coral_cli::run(argv(&[
        "--json",
        "spell",
        "--words",
        &words_file,
        "Frobnicate gix",
    ]))
    .await;
    assert_eq!(words(&before.json), ["Frobnicate"]);

    let added = coral_cli::run(argv(&[
        "--json",
        "spell",
        "--words",
        &words_file,
        "--add",
        "frobnicate",
        "rerere",
    ]))
    .await;
    assert_eq!(added.json["ok"], true, "{}", added.json);
    assert_eq!(
        added.json["result"]["words"],
        serde_json::json!(["frobnicate", "rerere"])
    );
    assert_eq!(
        std::fs::read_to_string(&words_file).unwrap(),
        "frobnicate\nrerere\n"
    );

    let after = coral_cli::run(argv(&[
        "--json",
        "spell",
        "--words",
        &words_file,
        "Frobnicate gix",
    ]))
    .await;
    assert_eq!(words(&after.json), Vec::<String>::new());
}

#[tokio::test]
async fn refuses_to_add_a_phrase() {
    let dir = tempfile::tempdir().unwrap();
    let out = coral_cli::run(argv(&[
        "--json",
        "spell",
        "--words",
        &list(&dir),
        "--add",
        "two words",
    ]))
    .await;
    assert_eq!(out.json["ok"], false);
    assert_eq!(out.json["error"]["code"], "refused");
    assert_eq!(
        format!("{:?}", out.code),
        format!("{:?}", std::process::ExitCode::from(1))
    );
}

#[tokio::test]
async fn refuses_a_dialect_it_has_no_dictionary_for() {
    let out = coral_cli::run(argv(&["--json", "spell", "--dialect", "fr-FR", "Bonjour"])).await;
    assert_eq!(out.json["error"]["code"], "usage");
    assert_eq!(
        format!("{:?}", out.code),
        format!("{:?}", std::process::ExitCode::from(2))
    );
}

#[tokio::test]
async fn reads_a_commit_message_file_as_git_leaves_it_for_a_hook() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("COMMIT_EDITMSG");
    std::fs::write(
        &file,
        "Fix teh watcher\n\
         # Please enter the commit message for your changes. Lines starting\n\
         # with '#' will be ignored, and an emptty message aborts the commit.\n\
         Explain it recieves events.\n\
         # ------------------------ >8 ------------------------\n\
         # Do not modify or remove the line above.\n\
         diff --git a/watchr.rs b/watchr.rs\n\
         +let seperate = 1;\n",
    )
    .unwrap();
    let out = coral_cli::run(argv(&[
        "--json",
        "spell",
        "--words",
        &list(&dir),
        "-F",
        file.to_str().unwrap(),
    ]))
    .await;
    let found = &out.json["result"]["misspellings"];
    assert_eq!(words(&out.json), ["teh", "recieves"]);
    // Line numbers are the file's, comment lines and all.
    assert_eq!(found[0]["line"], 1);
    assert_eq!(found[0]["column"], 5);
    assert_eq!(found[1]["line"], 4);
    assert_eq!(found[1]["column"], 12);
}

#[test]
fn reads_the_message_from_stdin() {
    let dir = tempfile::tempdir().unwrap();
    for args in [vec!["--json", "spell"], vec!["--json", "spell", "-F", "-"]] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_coral"))
            .args(&args)
            .args(["--words", &list(&dir)])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"Fix teh watcher\n\nIt recieves events.\n")
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{args:?}");
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(words(&json), ["teh", "recieves"], "{args:?}");
    }
}

/// Each run is a new process with a new dictionary, and suggestions that score the same once
/// came out in a different order every time.
#[test]
fn suggests_the_same_words_in_the_same_order_every_run() {
    let dir = tempfile::tempdir().unwrap();
    let run = || {
        let output = Command::new(env!("CARGO_BIN_EXE_coral"))
            .args(["--json", "spell", "--words", &list(&dir)])
            .arg("colour recieves seperate wether occured")
            .output()
            .unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap()
    };
    let first = run();
    for _ in 0..4 {
        assert_eq!(run(), first);
    }
}
