//! Staging, including partial staging. Every case checks the resulting *content* of the index
//! and worktree against git, not just that the command exited zero: a patch that stages the
//! wrong lines applies perfectly cleanly.

use std::fmt::Write as _;

use coral_core::diff::DiffOptions;
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
    loc.diff(&runner, false, &[], DiffOptions::default())
        .await
        .unwrap()
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
    let staged = loc
        .diff(&runner, true, &[], DiffOptions::default())
        .await
        .unwrap();
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

/// Discarding takes the change out of the working tree and leaves the index where it was.
#[test]
fn discarding_one_hunk_leaves_the_index_alone() {
    let repo = TestRepo::new()
        .write(
            "a.txt",
            "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\n",
        )
        .commit("base");
    // Two changes far enough apart to be separate hunks.
    std::fs::write(
        repo.path().join("a.txt"),
        "ONE\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nTEN\n",
    )
    .unwrap();

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();

        let files = loc
            .diff(&runner, false, &["a.txt"], DiffOptions::default())
            .await
            .unwrap();
        assert_eq!(files[0].hunks.len(), 2, "two hunks to choose between");

        let patch =
            build_patch(&files[0], &[(0, Selection::WholeHunk)], Direction::Discard).unwrap();
        loc.apply_to_index(&runner, &patch, Direction::Discard)
            .await
            .unwrap();

        // The first change is gone from the file; the second is still there.
        let on_disk = std::fs::read_to_string(repo.path().join("a.txt")).unwrap();
        assert!(
            on_disk.starts_with("one\n"),
            "the discarded change came back"
        );
        assert!(on_disk.ends_with("TEN\n"), "the other change went too");
        // And nothing was staged along the way.
        assert_eq!(repo.git(["diff", "--cached", "--name-only"]), "");
    });
}

/// A path git has to quote, staged one hunk at a time.
///
/// The header was written raw, so `diff --git a/new\nline.txt …` became two lines and
/// `git apply` refused with "diff header lacks filename information", staging nothing. A tab
/// was worse: git reads `--- ` up to the first tab, so the name silently lost its second half.
// Unix only: every name below is one Windows refuses outright, which is why git has to quote
// them in the first place.
#[cfg(unix)]
#[tokio::test]
async fn stages_a_hunk_of_a_file_whose_name_git_has_to_quote() {
    for name in ["new\nline.txt", "tab\tsep.txt", "quote\"and\\slash.txt"] {
        let repo = TestRepo::new().write(name, "one\n").commit("base");
        let repo = repo.write(name, "one\ntwo\n");
        let (runner, loc) = open(&repo).await;

        let files = unstaged_diff(&repo).await;
        let file = files.iter().find(|f| f.path == name).expect(name);
        let patch = build_patch(file, &[(0, Selection::WholeHunk)], Direction::Stage).unwrap();
        // Exactly what git writes for the same file, so the two cannot drift.
        let theirs = repo.git_bytes(["diff", "--", name]);
        let header = |p: &[u8]| p.split(|b| *b == b'\n').next().unwrap_or_default().to_vec();
        assert_eq!(header(&patch), header(&theirs), "header for {name:?}");

        loc.apply_to_index(&runner, &patch, Direction::Stage)
            .await
            .unwrap_or_else(|e| panic!("apply for {name:?}: {e}"));
        assert_eq!(repo.git(["show", &format!(":{name}")]), "one\ntwo");
    }
}

/// Content whose exact bytes a rebuilt patch can quietly change.
///
/// A file with no trailing newline, one with CRLF endings, and a symlink are all rebuilt from
/// the parsed hunk rather than copied, so each is a chance to add a newline that was not
/// there, normalise a line ending, or write the target as ordinary text. The index blob is
/// compared byte for byte, since every one of these applies perfectly cleanly while being
/// wrong.
#[tokio::test]
async fn stages_a_hunk_without_changing_bytes_it_was_not_asked_to() {
    let repo = TestRepo::new()
        .write("nonl.txt", "no newline at end")
        .write("crlf.txt", "a\r\nb\r\nc\r\n")
        .commit("base");
    let repo = repo
        .write("nonl.txt", "no newline at end, changed")
        .write("crlf.txt", "a\r\nB\r\nc\r\n");
    let (runner, loc) = open(&repo).await;

    for name in ["nonl.txt", "crlf.txt"] {
        let files = unstaged_diff(&repo).await;
        let file = files.iter().find(|f| f.path == name).expect(name);
        let patch = build_patch(file, &[(0, Selection::WholeHunk)], Direction::Stage).unwrap();
        loc.apply_to_index(&runner, &patch, Direction::Stage)
            .await
            .unwrap_or_else(|e| panic!("apply for {name}: {e}"));
    }

    // `git show :path` through the fixture trims, so the bytes are read with cat-file.
    let bytes = |spec: &str| repo.git_bytes(["cat-file", "blob", spec]);
    assert_eq!(bytes(":nonl.txt"), b"no newline at end, changed");
    assert_eq!(bytes(":crlf.txt"), b"a\r\nB\r\nc\r\n");
}

#[tokio::test]
async fn unstages_a_single_line_out_of_several() {
    // The window offers "Unstage 1 line" beside "Unstage hunk", and every use of it was
    // refused: the patch was written with the unselected additions left out and the unselected
    // removals kept, which describes the side the index does not hold.
    let repo = TestRepo::new().write("f.txt", "a\nz\n").commit("base");
    let repo = repo.write("f.txt", "a\nfirst\nsecond\nthird\nz\n");
    let (runner, loc) = open(&repo).await;
    loc.stage(&runner, &["f.txt"]).await.unwrap();

    let staged = loc
        .diff(&runner, true, &[], DiffOptions::default())
        .await
        .unwrap();
    let f = &staged[0];
    let second = f.hunks[0]
        .lines
        .iter()
        .position(|l| l.text == "second")
        .unwrap();

    let patch = build_patch(
        f,
        &[(0, Selection::Lines(vec![second]))],
        Direction::Unstage,
    )
    .unwrap();
    loc.apply_to_index(&runner, &patch, Direction::Unstage)
        .await
        .unwrap();

    assert_eq!(
        staged_content(&repo, "f.txt"),
        "a\nfirst\nthird\nz",
        "only the line asked for came back out of the index"
    );
}

#[tokio::test]
async fn unstages_a_single_removal_out_of_several() {
    let repo = TestRepo::new()
        .write("f.txt", "keep\ndrop me\nalso drop\ntail\n")
        .commit("base");
    let repo = repo.write("f.txt", "keep\ntail\n");
    let (runner, loc) = open(&repo).await;
    loc.stage(&runner, &["f.txt"]).await.unwrap();

    let staged = loc
        .diff(&runner, true, &[], DiffOptions::default())
        .await
        .unwrap();
    let f = &staged[0];
    let first = f.hunks[0]
        .lines
        .iter()
        .position(|l| l.text == "drop me")
        .unwrap();

    let patch = build_patch(f, &[(0, Selection::Lines(vec![first]))], Direction::Unstage).unwrap();
    loc.apply_to_index(&runner, &patch, Direction::Unstage)
        .await
        .unwrap();

    assert_eq!(
        staged_content(&repo, "f.txt"),
        "keep\ndrop me\ntail",
        "the line whose removal was taken back is in the index again"
    );
}

#[tokio::test]
async fn discards_a_single_line_out_of_several() {
    let repo = TestRepo::new().write("f.txt", "a\nz\n").commit("base");
    let repo = repo.write("f.txt", "a\nfirst\nsecond\nthird\nz\n");

    let files = unstaged_diff(&repo).await;
    let f = &files[0];
    let second = f.hunks[0]
        .lines
        .iter()
        .position(|l| l.text == "second")
        .unwrap();

    let patch = build_patch(
        f,
        &[(0, Selection::Lines(vec![second]))],
        Direction::Discard,
    )
    .unwrap();
    let (runner, loc) = open(&repo).await;
    loc.apply_to_index(&runner, &patch, Direction::Discard)
        .await
        .unwrap();

    assert_eq!(
        std::fs::read_to_string(repo.path().join("f.txt")).unwrap(),
        "a\nfirst\nthird\nz\n",
        "the one line went, and the two beside it stayed"
    );
}

#[tokio::test]
async fn discards_a_single_removal_out_of_several() {
    let repo = TestRepo::new()
        .write("f.txt", "keep\ndrop me\nalso drop\ntail\n")
        .commit("base");
    let repo = repo.write("f.txt", "keep\ntail\n");

    let files = unstaged_diff(&repo).await;
    let f = &files[0];
    let first = f.hunks[0]
        .lines
        .iter()
        .position(|l| l.text == "drop me")
        .unwrap();

    let patch = build_patch(f, &[(0, Selection::Lines(vec![first]))], Direction::Discard).unwrap();
    let (runner, loc) = open(&repo).await;
    loc.apply_to_index(&runner, &patch, Direction::Discard)
        .await
        .unwrap();

    assert_eq!(
        std::fs::read_to_string(repo.path().join("f.txt")).unwrap(),
        "keep\ndrop me\ntail\n",
        "the line whose deletion was undone is back, the other is still gone"
    );
}
