//! Making a repository from the terminal.
//!
//! These exist as much for the engine as for the CLI: cloning had no headless test surface at
//! all, so the only way to exercise it end to end was to drive the window at a real host.

use std::ffi::OsString;

use coral_core::testutil::TestRepo;

fn argv(args: &[&str]) -> Vec<OsString> {
    std::iter::once("coral")
        .chain(args.iter().copied())
        .map(OsString::from)
        .collect()
}

#[tokio::test]
async fn init_makes_a_repository_git_can_open() {
    let dir = tempfile::tempdir().unwrap();
    let at = dir.path().join("fresh");

    let out = coral_cli::run(argv(&[
        "--json",
        "init",
        at.to_str().unwrap(),
        "--branch",
        "trunk",
    ]))
    .await;

    assert_eq!(out.json["ok"], true, "{}", out.json);
    assert!(at.join(".git").is_dir());
    let head = std::fs::read_to_string(at.join(".git/HEAD")).unwrap();
    assert!(head.contains("refs/heads/trunk"), "{head}");
}

#[tokio::test]
async fn init_refuses_to_write_over_a_repository() {
    let existing = TestRepo::new().write("a.txt", "1\n").commit("base");

    let out = coral_cli::run(argv(&["--json", "init", existing.path().to_str().unwrap()])).await;

    assert_eq!(out.json["ok"], false, "{}", out.json);
    assert_eq!(out.json["error"]["code"], "already_a_repository");
}

#[tokio::test]
async fn clone_lands_where_it_said_it_would() {
    let source = TestRepo::new().write("a.txt", "1\n").commit("base");
    let dir = tempfile::tempdir().unwrap();

    let out = coral_cli::run(argv(&[
        "--json",
        "clone",
        &format!("{}/.git", source.path().display()),
        "--into",
        dir.path().to_str().unwrap(),
        "--name",
        "landed",
        "--quiet",
    ]))
    .await;

    assert_eq!(out.json["ok"], true, "{}", out.json);
    let landed = dir.path().join("landed");
    assert_eq!(out.json["result"]["path"], landed.to_str().unwrap());
    assert!(landed.join("a.txt").exists());
}

#[tokio::test]
async fn clone_records_the_key_it_was_given() {
    // The window can choose a key; so can the terminal, and both go through the one builder,
    // so the `IdentitiesOnly=yes` and the `-F none` that make a pin a pin cannot be lost to a
    // second spelling of the command.
    let source = TestRepo::new().write("a.txt", "1\n").commit("base");
    let dir = tempfile::tempdir().unwrap();

    let out = coral_cli::run(argv(&[
        "--json",
        "clone",
        &format!("{}/.git", source.path().display()),
        "--into",
        dir.path().to_str().unwrap(),
        "--name",
        "keyed",
        "--ssh-key",
        "/home/someone/.ssh/id_work",
        "--quiet",
    ]))
    .await;

    assert_eq!(out.json["ok"], true, "{}", out.json);
    let written = std::fs::read_to_string(dir.path().join("keyed/.git/config")).unwrap();
    assert!(
        written.contains(&coral_core::ssh::command_for("/home/someone/.ssh/id_work")),
        "{written}"
    );
}

#[tokio::test]
async fn a_clone_that_cannot_reach_its_source_fails_and_leaves_nothing() {
    let dir = tempfile::tempdir().unwrap();

    let out = coral_cli::run(argv(&[
        "--json",
        "clone",
        "/does/not/exist/anywhere.git",
        "--into",
        dir.path().to_str().unwrap(),
        "--name",
        "nothing",
        "--quiet",
    ]))
    .await;

    assert_eq!(out.json["ok"], false, "{}", out.json);
    assert!(
        !dir.path().join("nothing").exists(),
        "a failed clone left a directory behind"
    );
}

#[tokio::test]
async fn clone_can_take_only_the_recent_history() {
    // The reason this is offered at all: somebody who wants to read the current tree of
    // something enormous should not wait for its whole history first.
    let source = TestRepo::new().write("a.txt", "1\n").commit("one");
    let source = source.write("a.txt", "2\n").commit("two");
    let dir = tempfile::tempdir().unwrap();

    let out = coral_cli::run(argv(&[
        "--json",
        "clone",
        // `file://`, since a plain path clone hardlinks the object store and ignores a depth.
        &format!("file://{}/.git", source.path().display()),
        "--into",
        dir.path().to_str().unwrap(),
        "--name",
        "shallow",
        "--depth",
        "1",
        "--quiet",
    ]))
    .await;

    assert_eq!(out.json["ok"], true, "{}", out.json);
    assert!(dir.path().join("shallow/.git/shallow").exists());
}

#[tokio::test]
async fn clone_can_leave_the_file_contents_on_the_server() {
    let source = TestRepo::new().write("a.txt", "1\n").commit("one");
    let dir = tempfile::tempdir().unwrap();

    let out = coral_cli::run(argv(&[
        "--json",
        "clone",
        &format!("file://{}/.git", source.path().display()),
        "--into",
        dir.path().to_str().unwrap(),
        "--name",
        "partial",
        "--blobless",
        "--quiet",
    ]))
    .await;

    assert_eq!(out.json["ok"], true, "{}", out.json);
    let config = std::fs::read_to_string(dir.path().join("partial/.git/config")).unwrap();
    assert!(config.contains("partialclonefilter"), "{config}");
    assert!(
        !dir.path().join("partial/.git/shallow").exists(),
        "history is whole; only the blobs are absent"
    );
}
