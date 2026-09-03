//! Staging, including partial staging. Every case checks the resulting *content* of the index
//! and worktree against git, not just that the command exited zero: a patch that stages the
//! wrong lines applies perfectly cleanly.

use std::fmt::Write as _;

use coral_core::diff::FileDiff;
use coral_core::index::{Direction, Selection, build_patch};
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

/// "line 1\nline 2\n…", long enough that two edits land in separate hunks.
fn numbered_lines(n: u32) -> String {
    (1..=n).fold(String::new(), |mut s, i| {
        let _ = writeln!(s, "line {i}");
        s
    })
}

async fn open(repo: &TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

/// Content of a file as the index currently holds it.
fn staged_content(repo: &TestRepo, path: &str) -> String {
    repo.git(["show", &format!(":{path}")])
}

async fn unstaged_diff(repo: &TestRepo) -> Vec<FileDiff> {
    let (runner, loc) = open(repo).await;
    loc.diff(&runner, false, &[]).await.unwrap()
}

#[tokio::test]
async fn stages_and_unstages_whole_files() {
    let repo = TestRepo::new().write("a.txt", "one\n").commit("base");
    let repo = repo.write("a.txt", "two\n").write("new.txt", "new\n");
    let (runner, loc) = open(&repo).await;

    loc.stage(&runner, &["a.txt", "new.txt"]).await.unwrap();
    assert_eq!(staged_content(&repo, "a.txt"), "two");
    assert_eq!(staged_content(&repo, "new.txt"), "new");

    loc.unstage(&runner, &["a.txt"]).await.unwrap();
    assert_eq!(
        staged_content(&repo, "a.txt"),
        "one",
        "the index went back to HEAD"
    );
    // Unstaging must not touch the worktree.
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "two\n"
    );
}

#[tokio::test]
async fn discards_worktree_changes_without_touching_the_index() {
    let repo = TestRepo::new().write("a.txt", "one\n").commit("base");
    let repo = repo.write("a.txt", "staged\n");
    let (runner, loc) = open(&repo).await;
    loc.stage(&runner, &["a.txt"]).await.unwrap();

    let repo = repo.write("a.txt", "staged\nplus worktree\n");
    loc.discard(&runner, &["a.txt"]).await.unwrap();

    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "staged\n"
    );
    assert_eq!(
        staged_content(&repo, "a.txt"),
        "staged",
        "the staged version survived"
    );
}

#[tokio::test]
async fn unstages_a_file_added_before_the_first_commit() {
    let repo = TestRepo::new().write("a.txt", "new\n");
    let (runner, loc) = open(&repo).await;

    loc.stage(&runner, &["a.txt"]).await.unwrap();
    // With an unborn HEAD there is nothing to restore from; this must still work.
    loc.unstage(&runner, &["a.txt"]).await.unwrap();

    let status = loc.status(&runner).await.unwrap();
    assert_eq!(status.entries.len(), 1);
    assert_eq!(
        status.entries[0].index,
        coral_core::status::Change::Untracked
    );
}

/// Two separated edits produce two hunks. Staging only the first must leave the second in the
/// worktree and out of the index.
#[tokio::test]
async fn stages_one_hunk_and_leaves_the_other() {
    let body = numbered_lines(30);
    let repo = TestRepo::new().write("f.txt", &body).commit("base");

    let edited = body
        .replace("line 2\n", "line 2 EDITED\n")
        .replace("line 28\n", "line 28 EDITED\n");
    let repo = repo.write("f.txt", &edited);

    let files = unstaged_diff(&repo).await;
    let f = &files[0];
    assert_eq!(f.hunks.len(), 2, "edits 26 lines apart are separate hunks");

    let patch = build_patch(f, &[(0, Selection::WholeHunk)], Direction::Stage).unwrap();
    let (runner, loc) = open(&repo).await;
    loc.apply_to_index(&runner, &patch, Direction::Stage)
        .await
        .unwrap();

    let staged = staged_content(&repo, "f.txt");
    assert!(
        staged.contains("line 2 EDITED"),
        "the first hunk was staged"
    );
    assert!(
        !staged.contains("line 28 EDITED"),
        "the second hunk was not"
    );
    // The worktree still has both.
    let worktree = std::fs::read_to_string(repo.path().join("f.txt")).unwrap();
    assert!(worktree.contains("line 2 EDITED") && worktree.contains("line 28 EDITED"));
}

/// The case that makes partial staging hard: an unselected removal must become context, not
/// vanish. If it vanishes the patch still applies, and quietly deletes the line.
#[tokio::test]
async fn stages_selected_lines_and_treats_unselected_removals_as_context() {
    let repo = TestRepo::new()
        .write("f.txt", "keep\ndrop me\nalso drop\ntail\n")
        .commit("base");
    let repo = repo.write("f.txt", "keep\ntail\n");

    let files = unstaged_diff(&repo).await;
    let f = &files[0];
    let hunk = &f.hunks[0];

    // Select only the first of the two removals.
    let first_removal = hunk
        .lines
        .iter()
        .position(|l| l.text == "drop me")
        .expect("the removal is in the hunk");
    let patch = build_patch(
        f,
        &[(0, Selection::Lines(vec![first_removal]))],
        Direction::Stage,
    )
    .unwrap();

    let (runner, loc) = open(&repo).await;
    loc.apply_to_index(&runner, &patch, Direction::Stage)
        .await
        .unwrap();

    assert_eq!(
        staged_content(&repo, "f.txt"),
        "keep\nalso drop\ntail",
        "only the selected line was removed; the other survived as context"
    );
}

#[tokio::test]
async fn stages_a_single_added_line_out_of_several() {
    let repo = TestRepo::new().write("f.txt", "a\nz\n").commit("base");
    let repo = repo.write("f.txt", "a\nfirst\nsecond\nz\n");

    let files = unstaged_diff(&repo).await;
    let f = &files[0];
    let second = f.hunks[0]
        .lines
        .iter()
        .position(|l| l.text == "second")
        .unwrap();

    let patch = build_patch(f, &[(0, Selection::Lines(vec![second]))], Direction::Stage).unwrap();
    let (runner, loc) = open(&repo).await;
    loc.apply_to_index(&runner, &patch, Direction::Stage)
        .await
        .unwrap();

    assert_eq!(
        staged_content(&repo, "f.txt"),
        "a\nsecond\nz",
        "the unselected addition was left out entirely"
    );
}

#[tokio::test]
async fn unstages_a_hunk_in_reverse() {
    let body = numbered_lines(30);
    let repo = TestRepo::new().write("f.txt", &body).commit("base");
    let edited = body
        .replace("line 2\n", "line 2 EDITED\n")
        .replace("line 28\n", "line 28 EDITED\n");
    let repo = repo.write("f.txt", &edited);

    let (runner, loc) = open(&repo).await;
    loc.stage(&runner, &["f.txt"]).await.unwrap();

    // Now diff the index against HEAD and reverse one hunk back out.
    let staged = loc.diff(&runner, true, &[]).await.unwrap();
    let f = &staged[0];
    assert_eq!(f.hunks.len(), 2);
    let patch = build_patch(f, &[(1, Selection::WholeHunk)], Direction::Unstage).unwrap();
    loc.apply_to_index(&runner, &patch, Direction::Unstage)
        .await
        .unwrap();

    let now = staged_content(&repo, "f.txt");
    assert!(
        now.contains("line 2 EDITED"),
        "the first hunk is still staged"
    );
    assert!(
        !now.contains("line 28 EDITED"),
        "the second was reversed out"
    );
}

/// A selection that cancels out produces no hunk, and applying it must not fail.
#[test]
fn a_selection_of_nothing_produces_an_empty_patch() {
    let file = FileDiff {
        path: "f.txt".into(),
        old_path: None,
        change: coral_core::diff::FileChange::Modified,
        binary: false,
        added: Some(1),
        removed: Some(0),
        hunks: vec![coral_core::diff::Hunk {
            header: "@@ -1,1 +1,2 @@".into(),
            old_start: 1,
            old_lines: 1,
            new_start: 1,
            new_lines: 2,
            lines: vec![
                coral_core::diff::Line {
                    kind: coral_core::diff::LineKind::Context,
                    text: "a".into(),
                    old_no: Some(1),
                    new_no: Some(1),
                    no_newline: false,
                },
                coral_core::diff::Line {
                    kind: coral_core::diff::LineKind::Add,
                    text: "b".into(),
                    old_no: None,
                    new_no: Some(2),
                    no_newline: false,
                },
            ],
        }],
        too_large: false,
    };

    let patch = build_patch(&file, &[(0, Selection::Lines(vec![]))], Direction::Stage).unwrap();
    assert!(
        coral_core::index::is_empty_patch(&patch),
        "no changes selected, so no hunk"
    );
}

#[test]
fn asking_for_a_hunk_that_does_not_exist_is_an_error() {
    let file = FileDiff {
        path: "f.txt".into(),
        old_path: None,
        change: coral_core::diff::FileChange::Modified,
        binary: false,
        added: Some(0),
        removed: Some(0),
        hunks: vec![],
        too_large: false,
    };
    assert!(build_patch(&file, &[(3, Selection::WholeHunk)], Direction::Stage).is_err());
}
