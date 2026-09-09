//! Finishing a conflicted operation, which is where the commit for a merge or a rebase
//! actually lands.
//!
//! The window runs it through `operation_step`, not through `repo_action`, so it needs its own
//! ref snapshot: without one a conflicted merge left nothing in the journal, and the next Undo
//! stepped past it to an older entry whose refs had moved.

use coral_app_lib as app;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;
use coral_core::undo::Journal;

/// Two branches that change the same line, merged so that the merge stops.
fn stopped_merge() -> TestRepo {
    let r = TestRepo::new().write("f.txt", "one\ntwo\n").commit("base");
    r.git(["checkout", "-q", "-b", "side"]);
    let r = r.write("f.txt", "one\nSIDE\n").commit("side");
    r.git(["checkout", "-q", "main"]);
    let r = r.write("f.txt", "one\nMAIN\n").commit("main");
    r.command(["merge", "side"]).output().expect("spawn git");
    r
}

#[tokio::test(flavor = "multi_thread")]
async fn finishing_a_merge_is_recorded_and_can_be_undone() {
    let repo = stopped_merge();
    let path = repo.path().display().to_string();
    let before = repo.git(["rev-parse", "main"]);

    repo.git(["checkout", "--ours", "f.txt"]);
    repo.git(["add", "f.txt"]);
    app::conflicts::operation_step(path.clone(), "continue".to_owned())
        .await
        .expect("the merge finishes");

    let merged = repo.git(["rev-parse", "main"]);
    assert_ne!(merged, before, "the merge moved the branch");

    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    let journal = Journal::load(&loc);
    let last = journal.entries.last().expect("an entry for the step");
    assert_eq!(last.label, "finish the merge");

    let said = loc.undo_step(&runner, true).await.expect("undo applies");
    assert_eq!(said, "undid finish the merge");
    assert_eq!(repo.git(["rev-parse", "main"]), before);
}

#[tokio::test(flavor = "multi_thread")]
async fn aborting_records_nothing_when_it_moved_nothing() {
    let repo = stopped_merge();
    let path = repo.path().display().to_string();

    app::conflicts::operation_step(path, "abort".to_owned())
        .await
        .expect("the merge aborts");

    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    // An abort puts every ref back where it was, and the journal only records what moved.
    assert!(Journal::load(&loc).entries.is_empty());
}

/// Labels carry a short object id, whatever the window sent.
///
/// Four of them did not: revert, cherry-pick, merge and checkout interpolated the revision
/// straight in, so a failure reported "revert 1f98d424e506cf1dcce211f7b42e967ab48bb485 stopped
/// on conflicts" while every other surface in the window showed eight characters.
#[tokio::test(flavor = "multi_thread")]
async fn a_label_never_carries_a_full_object_id() {
    let repo = TestRepo::new().write("f.txt", "one\n").commit("base");
    let repo = repo.write("f.txt", "two\n").commit("second");
    let path = repo.path().display().to_string();
    let full = repo.git(["rev-parse", "HEAD"]);
    assert_eq!(full.len(), 40);

    app::actions::run_action(
        &path,
        app::actions::Action::Revert {
            revs: vec![full.clone()],
        },
    )
    .await
    .expect("the revert applies");

    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    let last = Journal::load(&loc).entries.pop().expect("an entry");
    assert_eq!(last.label, format!("revert {}", &full[..8]));
}

/// Renaming a branch, which the engine and the CLI have always been able to do and the window
/// could not: the action did not exist, so the branch menu had no item to put it behind.
#[tokio::test(flavor = "multi_thread")]
async fn a_branch_can_be_renamed_from_the_window() {
    let repo = TestRepo::new().write("f.txt", "one\n").commit("base");
    repo.git(["branch", "feature"]);
    let path = repo.path().display().to_string();

    app::actions::run_action(
        &path,
        app::actions::Action::BranchRename {
            from: "feature".to_owned(),
            to: "feature/renamed".to_owned(),
        },
    )
    .await
    .expect("the rename applies");

    let branches = repo.git(["branch", "--format=%(refname:short)"]);
    assert!(branches.contains("feature/renamed"), "{branches}");
    assert!(!branches.contains("\nfeature\n"), "{branches}");

    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    let last = Journal::load(&loc).entries.pop().expect("an entry");
    assert_eq!(last.label, "rename feature to feature/renamed");
}

/// The activity log is what somebody reads afterwards to find out what happened.
///
/// A merge that stopped on conflicts, and a push the remote rejected, both come back as an
/// outcome rather than an error — and both were logged as "finished", so the record of a
/// rejected force push read exactly like the record of one that went through.
#[tokio::test(flavor = "multi_thread")]
async fn a_merge_that_stopped_is_not_logged_as_finished() {
    let repo = stopped_merge();
    let path = repo.path().display().to_string();
    // `stopped_merge` conflicts through plain git; do it again through the window so the log
    // has an entry for it.
    app::conflicts::operation_step(path.clone(), "abort".to_owned())
        .await
        .expect("the merge aborts");
    let outcome = app::actions::run_action(
        &path,
        app::actions::Action::Merge {
            rev: "side".to_owned(),
            mode: coral_core::ops::MergeMode::NoFf,
        },
    )
    .await
    .expect("the merge runs");
    assert!(outcome.conflicted, "the fixture is built to conflict");

    let said: Vec<String> = app::activity::entries(Some(&path))
        .into_iter()
        .map(|e| e.message)
        .collect();
    let ended: Vec<&String> = said.iter().filter(|m| m.contains("merge side")).collect();
    assert!(
        ended.iter().any(|m| m.contains("did not complete")),
        "the log says what happened: {said:?}"
    );
    assert!(
        !ended.iter().any(|m| m.contains("finished")),
        "and does not also say it finished: {said:?}"
    );
}

/// A failure carries the name of the operation it belongs to.
///
/// Without it the window had nothing to title a red toast with but "Something went wrong",
/// which says only what the colour already said, and left git's own account of the failure to
/// do the whole job underneath it.
#[tokio::test(flavor = "multi_thread")]
async fn a_failed_action_says_which_operation_failed() {
    let repo = TestRepo::new().write("a.txt", "one\n").commit("first");
    let path = repo.path().display().to_string();

    let failed = app::actions::run_action(
        &path,
        app::actions::Action::TagCreate {
            name: "a release".to_owned(),
            at: None,
            message: None,
        },
    )
    .await
    .expect_err("git refuses a tag name with a space in it");

    assert_eq!(failed.what.as_deref(), Some("tag a release"));
    assert!(
        failed.message.contains("not a valid tag name"),
        "and still says why: {}",
        failed.message
    );
}
