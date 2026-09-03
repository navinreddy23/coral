//! Every shape `git diff` emits, checked against a live repository rather than only against
//! captured text, so the parser cannot drift from what git actually produces.

use coral_core::diff::{FileChange, LineKind};
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

/// One repository carrying every case at once: add, delete, modify, rename, mode change,
/// binary, no-trailing-newline, and a path containing a space.
fn every_shape() -> TestRepo {
    let r = TestRepo::new()
        .write("mod.txt", "one\ntwo\nthree\n")
        .write("del.txt", "gone\n")
        .write("ren.txt", "old name\n")
        .write("mode.txt", "x\n")
        .write("nonl.txt", "no newline at end")
        .write("with space.txt", "spaced\n")
        .commit("base");

    let r = r
        .write("mod.txt", "one\nTWO\nthree\nfour\n")
        .write("nonl.txt", "no newline CHANGED")
        .write("add.txt", "brand new\n")
        .write("with space.txt", "spaced and changed\n");
    r.git(["rm", "--quiet", "del.txt"]);
    r.git(["mv", "ren.txt", "ren-new.txt"]);
    std::fs::set_permissions(
        r.path().join("mode.txt"),
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .unwrap();
    r.git(["add", "--all"]);
    r
}

async fn staged(repo: &TestRepo) -> Vec<coral_core::diff::FileDiff> {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    loc.diff(&runner, true, &[]).await.unwrap()
}

#[tokio::test]
async fn classifies_every_kind_of_change() {
    let repo = every_shape();
    let files = staged(&repo).await;
    let find = |name: &str| {
        files
            .iter()
            .find(|f| f.path == name)
            .unwrap_or_else(|| panic!("{name} missing"))
    };

    assert_eq!(find("add.txt").change, FileChange::Added);
    assert_eq!(find("del.txt").change, FileChange::Deleted);
    assert_eq!(find("mod.txt").change, FileChange::Modified);
    assert_eq!(find("ren-new.txt").change, FileChange::Renamed);
    assert_eq!(
        find("ren-new.txt")
            .old_path
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some("ren.txt")
    );
}

/// `diff --git a/with space.txt b/with space.txt` cannot be split reliably, and git does not
/// quote for a plain space. Paths therefore come from the NUL-delimited numstat.
#[tokio::test]
async fn a_path_containing_a_space_survives() {
    let repo = every_shape();
    let files = staged(&repo).await;

    let f = files
        .iter()
        .find(|f| f.path == "with space.txt")
        .expect("spaced path missing");
    assert_eq!(f.change, FileChange::Modified);
    assert!(
        !f.hunks.is_empty(),
        "the spaced file should still have hunks"
    );
}

#[tokio::test]
async fn line_numbers_and_kinds_track_through_a_hunk() {
    let repo = every_shape();
    let files = staged(&repo).await;
    let f = files.iter().find(|f| f.path == "mod.txt").unwrap();

    assert_eq!(f.added, Some(2));
    assert_eq!(f.removed, Some(1));
    let h = &f.hunks[0];
    assert_eq!((h.old_start, h.new_start), (1, 1));

    let kinds: Vec<_> = h.lines.iter().map(|l| l.kind).collect();
    assert_eq!(
        kinds,
        vec![
            LineKind::Context,
            LineKind::Remove,
            LineKind::Add,
            LineKind::Context,
            LineKind::Add
        ]
    );
    // Context advances both sides; an add advances only the new side.
    assert_eq!((h.lines[0].old_no, h.lines[0].new_no), (Some(1), Some(1)));
    assert_eq!((h.lines[1].old_no, h.lines[1].new_no), (Some(2), None));
    assert_eq!((h.lines[2].old_no, h.lines[2].new_no), (None, Some(2)));
    assert_eq!((h.lines[4].old_no, h.lines[4].new_no), (None, Some(4)));
}

/// `\ No newline at end of file` annotates the line above rather than being a line itself.
#[tokio::test]
async fn a_missing_trailing_newline_annotates_its_line() {
    let repo = every_shape();
    let files = staged(&repo).await;
    let f = files.iter().find(|f| f.path == "nonl.txt").unwrap();

    let h = &f.hunks[0];
    assert!(h.lines.iter().all(|l| l.kind != LineKind::Context));
    assert!(
        h.lines.iter().all(|l| l.no_newline),
        "both sides lack a trailing newline, and neither backslash line became a line"
    );
    assert!(h.lines.iter().all(|l| !l.text.starts_with(b"\\")));
}

#[tokio::test]
async fn a_binary_file_reports_no_counts_and_no_hunks() {
    let repo = TestRepo::new();
    std::fs::write(repo.path().join("bin.dat"), [0_u8, 1, 2, 0, 255, 3]).unwrap();
    let repo = repo.commit("base");
    std::fs::write(repo.path().join("bin.dat"), [9_u8, 8, 0, 7, 0, 6]).unwrap();
    repo.git(["add", "--all"]);

    let files = staged(&repo).await;
    let f = &files[0];
    assert!(f.binary);
    assert_eq!((f.added, f.removed), (None, None));
    assert!(f.hunks.is_empty());
}

/// A mode change carries no hunks; the file must still be reported.
#[tokio::test]
async fn a_mode_only_change_is_reported_without_hunks() {
    let repo = every_shape();
    let files = staged(&repo).await;
    let f = files.iter().find(|f| f.path == "mode.txt").unwrap();

    assert_eq!((f.added, f.removed), (Some(0), Some(0)));
    assert!(f.hunks.is_empty());
}

#[tokio::test]
async fn hunks_land_on_the_right_files_when_several_change_at_once() {
    let repo = every_shape();
    let files = staged(&repo).await;

    for f in &files {
        for h in &f.hunks {
            for l in &h.lines {
                assert!(
                    !l.text.starts_with(b"diff --git"),
                    "{}: a section boundary leaked into a hunk",
                    f.path
                );
            }
        }
    }
    let modded = files.iter().find(|f| f.path == "mod.txt").unwrap();
    assert!(modded.hunks[0].lines.iter().any(|l| l.text == "TWO"));
    let added = files.iter().find(|f| f.path == "add.txt").unwrap();
    assert!(added.hunks[0].lines.iter().any(|l| l.text == "brand new"));
}

#[tokio::test]
async fn a_clean_repository_diffs_to_nothing() {
    let repo = TestRepo::new().write("a.txt", "a\n").commit("base");
    assert!(staged(&repo).await.is_empty());
}
