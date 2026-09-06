use coral_core::ops::{CommitOpts, ResetMode};
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;
use coral_core::undo::{Journal, JournalEntry, RefSnapshot};

async fn open(repo: &TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

fn entry(label: &str, before: RefSnapshot, after: RefSnapshot) -> JournalEntry {
    JournalEntry {
        label: label.to_owned(),
        before,
        after,
        at: 0,
    }
}

#[tokio::test]
async fn a_snapshot_captures_every_ref_and_where_head_points() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    repo.git(["branch", "feature"]);
    repo.git(["tag", "v1"]);
    let (runner, loc) = open(&repo).await;

    let snap = loc.snapshot_refs(&runner).await.unwrap();
    assert!(snap.refs.contains_key("refs/heads/main"));
    assert!(snap.refs.contains_key("refs/heads/feature"));
    assert!(snap.refs.contains_key("refs/tags/v1"));
    assert_eq!(snap.head_branch.as_deref(), Some("main"));
    assert_eq!(
        snap.head_oid.as_deref(),
        Some(repo.git(["rev-parse", "HEAD"]).as_str())
    );
}

#[tokio::test]
async fn a_snapshot_diff_names_only_what_moved() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let (runner, loc) = open(&repo).await;
    let before = loc.snapshot_refs(&runner).await.unwrap();

    repo.git(["branch", "added"]);
    let _moved = repo.write("a.txt", "2\n").commit("moved main");
    let after = loc.snapshot_refs(&runner).await.unwrap();

    let changed: Vec<String> = before.diff(&after).into_iter().map(|(n, _, _)| n).collect();
    assert!(
        changed.contains(&"refs/heads/main".to_owned()),
        "main moved"
    );
    assert!(
        changed.contains(&"refs/heads/added".to_owned()),
        "a new ref counts as a change"
    );
    assert_eq!(changed.len(), 2);
}

/// Undoing a commit puts the branch back and leaves the tree as it was.
#[tokio::test]
async fn undoes_a_commit_by_restoring_refs() {
    let repo = TestRepo::new().write("a.txt", "one\n").commit("first");
    let (runner, loc) = open(&repo).await;
    let before = loc.snapshot_refs(&runner).await.unwrap();

    let repo = repo.write("a.txt", "two\n");
    loc.stage(&runner, &["a.txt"]).await.unwrap();
    loc.commit(
        &runner,
        &CommitOpts {
            message: "second".into(),
            ..CommitOpts::default()
        },
    )
    .await
    .unwrap();
    let after = loc.snapshot_refs(&runner).await.unwrap();
    assert_eq!(repo.git(["rev-list", "--count", "HEAD"]), "2");

    loc.restore_refs(&runner, &before, &after).await.unwrap();
    assert_eq!(repo.git(["rev-list", "--count", "HEAD"]), "1");
    assert_eq!(repo.git(["rev-parse", "HEAD"]), before.head_oid.unwrap());
}

#[tokio::test]
async fn undoes_a_branch_deletion_by_recreating_the_ref() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    repo.git(["branch", "doomed"]);
    let (runner, loc) = open(&repo).await;
    let before = loc.snapshot_refs(&runner).await.unwrap();

    loc.branch_delete(&runner, "doomed", true).await.unwrap();
    assert!(repo.git(["branch", "--list", "doomed"]).is_empty());
    let after = loc.snapshot_refs(&runner).await.unwrap();

    loc.restore_refs(&runner, &before, &after).await.unwrap();
    assert!(
        repo.git(["branch", "--list", "doomed"]).contains("doomed"),
        "the branch is back"
    );
}

#[tokio::test]
async fn undoes_a_hard_reset() {
    let repo = TestRepo::new().write("a.txt", "one\n").commit("first");
    let repo = repo.write("a.txt", "two\n").commit("second");
    let (runner, loc) = open(&repo).await;
    let before = loc.snapshot_refs(&runner).await.unwrap();

    loc.reset(&runner, "HEAD~1", ResetMode::Hard).await.unwrap();
    assert_eq!(repo.git(["rev-list", "--count", "HEAD"]), "1");
    let after = loc.snapshot_refs(&runner).await.unwrap();

    loc.restore_refs(&runner, &before, &after).await.unwrap();
    assert_eq!(repo.git(["rev-list", "--count", "HEAD"]), "2");
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "two\n"
    );
}

/// Moving refs underneath uncommitted work silently changes what that work means, so undo
/// refuses rather than doing it.
#[tokio::test]
async fn refuses_to_undo_when_the_worktree_is_dirty() {
    let repo = TestRepo::new().write("a.txt", "one\n").commit("first");
    let (runner, loc) = open(&repo).await;
    let before = loc.snapshot_refs(&runner).await.unwrap();
    let repo = repo.write("a.txt", "two\n").commit("second");
    let after = loc.snapshot_refs(&runner).await.unwrap();

    std::fs::write(repo.path().join("a.txt"), "uncommitted work\n").unwrap();
    let err = loc
        .restore_refs(&runner, &before, &after)
        .await
        .unwrap_err();

    assert_eq!(err.code(), "refused");
    assert_eq!(
        repo.git(["rev-list", "--count", "HEAD"]),
        "2",
        "nothing was moved"
    );
}

#[test]
fn the_journal_tracks_what_can_be_undone_and_redone() {
    let mut j = Journal::default();
    assert!(j.undoable().is_none());
    assert!(j.redoable().is_none());

    j.record(entry(
        "first",
        RefSnapshot::default(),
        RefSnapshot::default(),
    ));
    j.record(entry(
        "second",
        RefSnapshot::default(),
        RefSnapshot::default(),
    ));

    assert_eq!(j.undoable().unwrap().label, "second");
    j.undone += 1;
    assert_eq!(j.undoable().unwrap().label, "first");
    assert_eq!(j.redoable().unwrap().label, "second");

    j.undone += 1;
    assert!(j.undoable().is_none(), "nothing left to undo");
    assert_eq!(j.redoable().unwrap().label, "first");
}

/// Doing something new after an undo abandons the redo stack, as every editor does.
#[test]
fn recording_after_an_undo_discards_the_redo_stack() {
    let mut j = Journal::default();
    j.record(entry("a", RefSnapshot::default(), RefSnapshot::default()));
    j.record(entry("b", RefSnapshot::default(), RefSnapshot::default()));
    j.undone = 1;

    j.record(entry("c", RefSnapshot::default(), RefSnapshot::default()));

    assert_eq!(j.undone, 0);
    assert_eq!(j.entries.len(), 2);
    assert_eq!(j.entries[1].label, "c", "b was abandoned");
    assert!(j.redoable().is_none());
}

#[test]
fn the_journal_is_bounded() {
    let mut j = Journal::default();
    for i in 0..60 {
        j.record(entry(
            &format!("op {i}"),
            RefSnapshot::default(),
            RefSnapshot::default(),
        ));
    }
    assert_eq!(j.entries.len(), 50);
    assert_eq!(
        j.entries[0].label, "op 10",
        "the oldest entries were dropped"
    );
}

#[tokio::test]
async fn the_journal_round_trips_through_the_git_dir() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let (runner, loc) = open(&repo).await;

    let mut j = Journal::default();
    let snap = loc.snapshot_refs(&runner).await.unwrap();
    j.record(entry("commit", snap.clone(), snap));
    j.save(&loc).unwrap();

    let loaded = Journal::load(&loc);
    assert_eq!(loaded.entries.len(), 1);
    assert_eq!(loaded.entries[0].label, "commit");
    // It lives in the git dir, so it is never committed.
    assert!(Journal::path(&loc).starts_with(&loc.git_dir));
}

/// A corrupt undo stack must never stop a repository from opening.
#[tokio::test]
async fn a_corrupt_journal_loads_as_empty() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let (_, loc) = open(&repo).await;

    let path = Journal::path(&loc);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"{ not json at all").unwrap();

    assert!(Journal::load(&loc).entries.is_empty());
}

/// A ref that moved outside Coral makes the step unsafe, and saying so is the whole point.
///
/// `update-ref` catches this too, and refuses with "cannot lock ref 'refs/heads/main': is at
/// <sha> but expected <sha>" — which is git explaining itself to somebody who pressed Undo.
#[tokio::test]
async fn undo_refuses_plainly_once_a_ref_has_moved_underneath_it() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let (runner, loc) = open(&repo).await;

    let before = loc.snapshot_refs(&runner).await.unwrap();
    let repo = repo.write("a.txt", "2\n").commit("second");
    let after = loc.snapshot_refs(&runner).await.unwrap();
    let mut journal = Journal::load(&loc);
    journal.record(entry("second commit", before, after));
    journal.save(&loc).unwrap();

    // Somebody commits in a terminal beside the window.
    let repo = repo.write("a.txt", "3\n").commit("third");

    let refused = loc.undo_step(&runner, true).await.unwrap_err();
    let said = refused.to_string();
    assert!(said.contains("refs/heads/main"), "names the ref: {said}");
    assert!(said.contains("moved since"), "says what happened: {said}");
    assert!(!said.contains("cannot lock"), "not git's words: {said}");
    // And nothing was touched on the way to refusing.
    assert_eq!(
        repo.git(["rev-parse", "HEAD"]),
        repo.git(["rev-parse", "main"])
    );
    assert_eq!(repo.git(["log", "--oneline", "-1", "--format=%s"]), "third");
}
