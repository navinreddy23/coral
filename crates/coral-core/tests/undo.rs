use coral_core::ops::{CommitOpts, ResetMode};
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;
use coral_core::undo::{Journal, JournalEntry, RefSnapshot, Restore};

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
        restore: Restore::Worktree,
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

    loc.restore_refs(&runner, &before, &after, Restore::Worktree, "undo")
        .await
        .unwrap();
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

    loc.restore_refs(&runner, &before, &after, Restore::Worktree, "undo")
        .await
        .unwrap();
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

    loc.restore_refs(&runner, &before, &after, Restore::Worktree, "undo")
        .await
        .unwrap();
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
        .restore_refs(&runner, &before, &after, Restore::Worktree, "undo")
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

/// The one action people most want back used to be the one that could never be taken back.
///
/// A soft or a mixed reset leaves the worktree dirty by construction, and undo refused on a
/// dirty worktree, so undoing a reset was refused every single time.
#[tokio::test]
async fn undoes_a_soft_reset_even_though_it_leaves_the_worktree_dirty() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let repo = repo.write("a.txt", "2\n").commit("second");
    let (runner, loc) = open(&repo).await;
    let at_second = repo.git(["rev-parse", "HEAD"]);

    let before = loc.snapshot_refs(&runner).await.unwrap();
    loc.reset(&runner, "HEAD~1", ResetMode::Soft).await.unwrap();
    let after = loc.snapshot_refs(&runner).await.unwrap();
    let mut journal = Journal::load(&loc);
    journal.record(entry("reset to HEAD~1", before, after));
    journal.save(&loc).unwrap();

    // The reset left its own output staged, which is what the refusal used to catch.
    assert!(!loc.status(&runner).await.unwrap().is_clean());

    let said = loc.undo_step(&runner, true).await.expect("undo applies");
    assert_eq!(said, "undid reset to HEAD~1");
    assert_eq!(repo.git(["rev-parse", "HEAD"]), at_second);
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "2\n"
    );
}

/// A redo that the same guard stops says so in its own words.
#[tokio::test]
async fn a_redo_the_worktree_blocks_is_refused_as_a_redo() {
    // The guard lives in `restore_refs`, which serves both directions and labelled every
    // refusal "undo". Pressing Redo and being told "cannot undo" names the wrong button.
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let repo = repo.write("a.txt", "2\n").commit("second");
    let (runner, loc) = open(&repo).await;

    let before = loc.snapshot_refs(&runner).await.unwrap();
    loc.reset(&runner, "HEAD~1", ResetMode::Soft).await.unwrap();
    let after = loc.snapshot_refs(&runner).await.unwrap();
    let mut journal = Journal::load(&loc);
    journal.record(entry("reset to HEAD~1", before, after));
    journal.save(&loc).unwrap();

    loc.undo_step(&runner, true).await.unwrap();
    std::fs::write(repo.path().join("a.txt"), "written by hand\n").unwrap();

    let refused = loc.undo_step(&runner, false).await.unwrap_err();
    assert_eq!(refused.code(), "refused", "{refused}");
    assert!(refused.to_string().contains("redo"), "{refused}");
    assert!(!refused.to_string().contains("undo"), "{refused}");
}

/// And work the user actually wrote still stops it, which is the whole reason for the guard.
#[tokio::test]
async fn refuses_when_the_worktree_holds_something_the_undo_would_overwrite() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let repo = repo.write("a.txt", "2\n").commit("second");
    let (runner, loc) = open(&repo).await;

    let before = loc.snapshot_refs(&runner).await.unwrap();
    loc.reset(&runner, "HEAD~1", ResetMode::Soft).await.unwrap();
    let after = loc.snapshot_refs(&runner).await.unwrap();
    let mut journal = Journal::load(&loc);
    journal.record(entry("reset to HEAD~1", before, after));
    journal.save(&loc).unwrap();

    // Now something that is not the reset's own output.
    std::fs::write(repo.path().join("a.txt"), "written by hand\n").unwrap();

    let refused = loc.undo_step(&runner, true).await.unwrap_err();
    assert!(
        refused.to_string().contains("worktree has changes"),
        "{refused}"
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "written by hand\n"
    );
}

/// Work that exists only in the index, which the worktree comparison cannot see.
#[tokio::test]
async fn refuses_when_the_only_copy_of_the_work_is_staged() {
    let repo = TestRepo::new().write("f.txt", "one\n").commit("base");
    let repo = repo.write("f.txt", "two\n").commit("second");
    let (runner, loc) = open(&repo).await;

    let before = loc.snapshot_refs(&runner).await.unwrap();
    loc.reset(&runner, "HEAD~1", ResetMode::Soft).await.unwrap();
    let after = loc.snapshot_refs(&runner).await.unwrap();
    let mut journal = Journal::load(&loc);
    journal.record(entry("reset to HEAD~1", before, after));
    journal.save(&loc).unwrap();

    // Author something, stage it, then put the file back to what the undo target holds. The
    // staged blob is now the only copy of it anywhere.
    std::fs::write(repo.path().join("f.txt"), "PRECIOUS WORK\n").unwrap();
    repo.git(["add", "f.txt"]);
    std::fs::write(repo.path().join("f.txt"), "two\n").unwrap();
    assert_eq!(repo.git(["show", ":f.txt"]), "PRECIOUS WORK");

    let stepped = loc.undo_step(&runner, true).await;
    assert_eq!(
        repo.git(["show", ":f.txt"]),
        "PRECIOUS WORK",
        "the staged work survived; step said {stepped:?}"
    );
}

/// A mixed reset, whose index holds the tree of the commit HEAD is on now.
///
/// That is the one difference from the target the guard allows, because it is a commit that
/// still exists: nothing in the index is the only copy of anything.
#[tokio::test]
async fn undoes_a_mixed_reset_whose_index_holds_a_commit_that_still_exists() {
    let repo = TestRepo::new().write("f.txt", "one\n").commit("base");
    let repo = repo.write("f.txt", "two\n").commit("second");
    let (runner, loc) = open(&repo).await;
    let at_second = repo.git(["rev-parse", "HEAD"]);

    let before = loc.snapshot_refs(&runner).await.unwrap();
    loc.reset(&runner, "HEAD~1", ResetMode::Mixed)
        .await
        .unwrap();
    let after = loc.snapshot_refs(&runner).await.unwrap();
    let mut journal = Journal::load(&loc);
    journal.record(entry("reset to HEAD~1", before, after));
    journal.save(&loc).unwrap();

    assert!(!loc.status(&runner).await.unwrap().is_clean());
    assert_eq!(
        repo.git(["show", ":f.txt"]),
        "one",
        "the index went back with HEAD"
    );

    loc.undo_step(&runner, true).await.expect("undo applies");
    assert_eq!(repo.git(["rev-parse", "HEAD"]), at_second);
    assert_eq!(
        std::fs::read_to_string(repo.path().join("f.txt")).unwrap(),
        "two\n"
    );
}

#[tokio::test]
async fn undoing_a_commit_gives_the_work_back_rather_than_destroying_it() {
    // The reason `Restore` exists. Reversing a merge means the files must match the commit
    // that comes back, and doing that to a commit would delete exactly what the user was
    // trying to recover. This is the assertion that stands between the two.
    let repo = TestRepo::new().write("a.txt", "one\n").commit("base");
    let (runner, loc) = open(&repo).await;
    let base = repo.git(["rev-parse", "HEAD"]);

    std::fs::write(repo.path().join("a.txt"), "one\ntwo\n").unwrap();
    std::fs::write(repo.path().join("new.txt"), "brand new\n").unwrap();
    repo.git(["add", "-A"]);

    let before = loc.snapshot_refs(&runner).await.unwrap();
    loc.commit(
        &runner,
        &CommitOpts {
            message: "the commit being undone".to_owned(),
            ..CommitOpts::default()
        },
    )
    .await
    .unwrap();
    let after = loc.snapshot_refs(&runner).await.unwrap();

    loc.restore_refs(&runner, &before, &after, Restore::KeepChanges, "undo")
        .await
        .unwrap();

    assert_eq!(
        repo.git(["rev-parse", "HEAD"]),
        base,
        "the branch stepped back"
    );
    // The whole point: the work is still on disk and still staged, exactly as it was a moment
    // before the commit.
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "one\ntwo\n"
    );
    assert!(
        repo.path().join("new.txt").exists(),
        "the new file survived"
    );
    let staged = repo.git(["diff", "--cached", "--name-only"]);
    assert!(staged.contains("a.txt"), "{staged}");
    assert!(staged.contains("new.txt"), "{staged}");
}

#[tokio::test]
async fn undoing_a_commit_is_allowed_over_later_edits() {
    // A soft reset overwrites nothing, so the guard that protects a hard one would only ever
    // refuse the case where undoing is always safe.
    let repo = TestRepo::new().write("a.txt", "one\n").commit("base");
    let (runner, loc) = open(&repo).await;

    std::fs::write(repo.path().join("a.txt"), "one\ntwo\n").unwrap();
    repo.git(["add", "-A"]);
    let before = loc.snapshot_refs(&runner).await.unwrap();
    loc.commit(
        &runner,
        &CommitOpts {
            message: "committed".to_owned(),
            ..CommitOpts::default()
        },
    )
    .await
    .unwrap();
    let after = loc.snapshot_refs(&runner).await.unwrap();

    // Carrying on working after committing, which is the normal thing to do.
    std::fs::write(repo.path().join("later.txt"), "written afterwards\n").unwrap();

    loc.restore_refs(&runner, &before, &after, Restore::KeepChanges, "undo")
        .await
        .unwrap();

    assert!(
        repo.path().join("later.txt").exists(),
        "work done after the commit must survive undoing it"
    );
}

#[tokio::test]
async fn undoing_a_merge_still_matches_the_files_to_the_commit() {
    // The other half of the same decision. Nothing here may be softened by the change above:
    // a merge leaves files on disk that belong to the merge, and they have to go with it.
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    // Whatever the fixture's first branch is called; `init.defaultBranch` is the user's to set.
    let trunk = repo.git(["rev-parse", "--abbrev-ref", "HEAD"]);
    repo.git(["checkout", "-q", "-b", "side"]);
    std::fs::write(repo.path().join("only-on-side.txt"), "side\n").unwrap();
    repo.git(["add", "-A"]);
    repo.git(["commit", "-qm", "a commit only on the side branch"]);
    repo.git(["checkout", "-q", &trunk]);

    let (runner, loc) = open(&repo).await;
    let before = loc.snapshot_refs(&runner).await.unwrap();
    repo.git(["merge", "-q", "--no-ff", "-m", "merge the side", "side"]);
    let after = loc.snapshot_refs(&runner).await.unwrap();
    assert!(repo.path().join("only-on-side.txt").exists());

    loc.restore_refs(&runner, &before, &after, Restore::Worktree, "undo")
        .await
        .unwrap();

    assert!(
        !repo.path().join("only-on-side.txt").exists(),
        "a file the merge brought in must go back with it"
    );
}

#[test]
fn a_journal_written_before_this_existed_still_loads() {
    // Every entry already on disk wants the old behaviour, which is the default, so an upgrade
    // must not turn an existing undo stack into an error.
    let old = r#"{"entries":[{"label":"Merge","before":{"refs":{},"headBranch":null,
        "headOid":null},"after":{"refs":{},"headBranch":null,"headOid":null},"at":0}],
        "undone":0}"#;
    let journal: Journal = serde_json::from_str(old).expect("an older journal still parses");
    assert_eq!(journal.entries[0].restore, Restore::Worktree);
}
