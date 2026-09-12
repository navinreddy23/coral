//! Every mutating operation, and the stop-resolve-continue and abort paths that the conflict
//! milestone depends on.

use coral_core::conflict::Resolution;
use coral_core::ops::{CommitOpts, MergeMode, OpAction, ResetMode};
use coral_core::process::GitRunner;
use coral_core::repo::{Head, OpState, RepoLocation};
use coral_core::testutil::TestRepo;

async fn open(repo: &TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

/// Two branches that changed the same line, so merging them conflicts.
fn conflicting() -> TestRepo {
    let r = TestRepo::new().write("f.txt", "base\n").commit("base");
    r.git(["checkout", "--quiet", "-b", "side"]);
    let r = r.write("f.txt", "side\n").commit("side change");
    r.git(["checkout", "--quiet", "main"]);
    r.write("f.txt", "main\n").commit("main change")
}

#[tokio::test]
async fn commits_amends_and_signs_off() {
    let repo = TestRepo::new().write("a.txt", "1\n");
    let (runner, loc) = open(&repo).await;
    loc.stage(&runner, &["a.txt"]).await.unwrap();

    let oid = loc
        .commit(
            &runner,
            &CommitOpts {
                message: "first".into(),
                ..CommitOpts::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(oid.len(), 40);
    assert_eq!(repo.git(["log", "-1", "--format=%s"]), "first");

    let amended = loc
        .commit(
            &runner,
            &CommitOpts {
                message: "first, reworded".into(),
                amend: true,
                signoff: true,
                ..CommitOpts::default()
            },
        )
        .await
        .unwrap();
    assert_ne!(amended, oid, "amending replaces the commit");
    assert_eq!(
        repo.git(["rev-list", "--count", "HEAD"]),
        "1",
        "and does not add one"
    );
    assert!(
        repo.git(["log", "-1", "--format=%b"])
            .contains("Signed-off-by")
    );
}

#[tokio::test]
async fn commits_with_an_overridden_author() {
    let repo = TestRepo::new().write("a.txt", "1\n");
    let (runner, loc) = open(&repo).await;
    loc.stage(&runner, &["a.txt"]).await.unwrap();

    loc.commit(
        &runner,
        &CommitOpts {
            message: "by someone else".into(),
            author: Some("Ada Lovelace <ada@example.com>".into()),
            ..CommitOpts::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(
        repo.git(["log", "-1", "--format=%an <%ae>"]),
        "Ada Lovelace <ada@example.com>"
    );
    assert_eq!(
        repo.git(["log", "-1", "--format=%cn"]),
        "Coral Fixture",
        "the committer is still us"
    );
}

#[tokio::test]
async fn creates_renames_checks_out_and_deletes_branches() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let (runner, loc) = open(&repo).await;

    loc.branch_create(&runner, "feature", None, false)
        .await
        .unwrap();
    assert!(
        repo.git(["branch", "--list", "feature"])
            .contains("feature")
    );

    loc.branch_rename(&runner, "feature", "renamed")
        .await
        .unwrap();
    loc.checkout(&runner, "renamed").await.unwrap();
    assert_eq!(
        loc.head(&runner).await.unwrap(),
        Head::Branch {
            name: "renamed".into()
        }
    );

    loc.checkout(&runner, "main").await.unwrap();
    loc.branch_delete(&runner, "renamed", false).await.unwrap();
    assert!(repo.git(["branch", "--list", "renamed"]).is_empty());
}

/// git refuses to delete an unmerged branch without force; that refusal must reach the caller.
#[tokio::test]
async fn refuses_to_delete_an_unmerged_branch_without_force() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let (runner, loc) = open(&repo).await;
    loc.branch_create(&runner, "work", None, true)
        .await
        .unwrap();
    let repo = repo.write("a.txt", "2\n").commit("unmerged work");
    loc.checkout(&runner, "main").await.unwrap();

    assert!(loc.branch_delete(&runner, "work", false).await.is_err());
    loc.branch_delete(&runner, "work", true).await.unwrap();
    assert!(repo.git(["branch", "--list", "work"]).is_empty());
}

#[tokio::test]
async fn creates_lightweight_and_annotated_tags() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let (runner, loc) = open(&repo).await;

    loc.tag_create(&runner, "light", None, None).await.unwrap();
    loc.tag_create(&runner, "heavy", None, Some("a release"))
        .await
        .unwrap();

    assert_eq!(repo.git(["cat-file", "-t", "light"]), "commit");
    assert_eq!(
        repo.git(["cat-file", "-t", "heavy"]),
        "tag",
        "a message makes it annotated"
    );

    loc.tag_delete(&runner, "light").await.unwrap();
    assert!(repo.git(["tag", "--list", "light"]).is_empty());
}

#[tokio::test]
async fn stashes_applies_pops_and_drops() {
    let repo = TestRepo::new().write("a.txt", "committed\n").commit("base");
    let repo = repo.write("a.txt", "work in progress\n");
    let (runner, loc) = open(&repo).await;

    loc.stash_push(&runner, Some("wip"), false).await.unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "committed\n"
    );
    assert!(repo.git(["stash", "list"]).contains("wip"));

    loc.stash_apply(&runner, 0, false).await.unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "work in progress\n"
    );
    assert!(
        !repo.git(["stash", "list"]).is_empty(),
        "apply keeps the entry"
    );

    loc.stash_drop(&runner, 0).await.unwrap();
    assert!(repo.git(["stash", "list"]).is_empty());
}

#[tokio::test]
async fn a_stash_that_lands_on_conflicts_has_stopped_rather_than_failed() {
    // git exits 1 either way. Reported as a failure it read "Something went wrong" in red, for
    // a situation git itself recovers from: both sides are in the file and the entry is kept.
    //
    // The message is the other half. git explains a conflicting pop on stdout and says nothing
    // at all on stderr, so a report that reads only stderr says that something failed and then
    // refuses to say what.
    let repo = TestRepo::new()
        .write(
            "a.txt", "base
",
        )
        .commit("base");
    let repo = repo.write(
        "a.txt", "stashed
",
    );
    let (runner, loc) = open(&repo).await;
    loc.stash_push(&runner, None, false).await.unwrap();
    let repo = repo
        .write(
            "a.txt",
            "committed over it
",
        )
        .commit("moved on");

    let outcome = loc
        .stash_apply(&runner, 0, true)
        .await
        .expect("a conflict is an outcome, not an error");

    assert!(!outcome.completed);
    assert_eq!(outcome.conflicts, ["a.txt"]);
    // Not `contains("CONFLICT")`. git 2.43 prints its CONFLICT lines through `--quiet` and
    // git 2.55 suppresses them, leaving only "The stash entry is kept in case you need it
    // again." What holds on both is that git says something rather than nothing, and that the
    // paths come from the repository rather than from git's prose, which is the assertion above.
    assert!(
        !outcome.message.trim().is_empty(),
        "git's own words reach the user: {}",
        outcome.message
    );
    assert!(
        std::fs::read_to_string(repo.path().join("a.txt"))
            .unwrap()
            .contains("<<<<<<<"),
        "both sides are in the file"
    );
    assert!(
        !repo.git(["stash", "list"]).is_empty(),
        "a pop that conflicted keeps the entry"
    );
}

#[tokio::test]
async fn a_stash_that_applies_cleanly_is_a_completed_outcome() {
    let repo = TestRepo::new()
        .write(
            "a.txt", "base
",
        )
        .commit("base");
    let repo = repo.write(
        "b.txt", "new
",
    );
    let (runner, loc) = open(&repo).await;
    loc.stash_push(&runner, None, true).await.unwrap();

    let outcome = loc.stash_apply(&runner, 0, true).await.unwrap();
    assert!(outcome.completed);
    assert!(outcome.conflicts.is_empty());
    assert!(
        repo.git(["stash", "list"]).is_empty(),
        "a clean pop drops it"
    );
}

#[tokio::test]
async fn stashes_untracked_files_only_when_asked() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let repo = repo.write("untracked.txt", "u\n");
    let (runner, loc) = open(&repo).await;

    loc.stash_push(&runner, None, false).await.unwrap();
    assert!(
        repo.path().join("untracked.txt").exists(),
        "left alone by default"
    );

    loc.stash_push(&runner, None, true).await.unwrap();
    assert!(
        !repo.path().join("untracked.txt").exists(),
        "taken with --include-untracked"
    );
}

#[tokio::test]
async fn resets_soft_mixed_and_hard() {
    let repo = TestRepo::new().write("a.txt", "one\n").commit("first");
    let repo = repo.write("a.txt", "two\n").commit("second");
    let (runner, loc) = open(&repo).await;

    loc.reset(&runner, "HEAD~1", ResetMode::Soft).await.unwrap();
    assert_eq!(repo.git(["rev-list", "--count", "HEAD"]), "1");
    assert_eq!(repo.git(["show", ":a.txt"]), "two", "soft keeps the index");

    loc.reset(&runner, "HEAD", ResetMode::Mixed).await.unwrap();
    assert_eq!(
        repo.git(["show", ":a.txt"]),
        "one",
        "mixed resets the index"
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "two\n",
        "but not the worktree"
    );

    loc.reset(&runner, "HEAD", ResetMode::Hard).await.unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "one\n"
    );
}

#[tokio::test]
async fn merges_cleanly_and_records_a_merge_commit() {
    let repo = TestRepo::new().write("shared.txt", "base\n").commit("base");
    repo.git(["checkout", "--quiet", "-b", "side"]);
    let repo = repo.write("side.txt", "s\n").commit("side");
    repo.git(["checkout", "--quiet", "main"]);
    let repo = repo.write("main.txt", "m\n").commit("main");

    let (runner, loc) = open(&repo).await;
    let outcome = loc
        .merge(&runner, "side", MergeMode::NoFf, Some("merge side"))
        .await
        .unwrap();

    assert!(outcome.completed);
    assert!(outcome.conflicts.is_empty());
    assert_eq!(loc.op_state(), OpState::Clean);
    assert_eq!(repo.git(["rev-list", "--count", "--merges", "HEAD"]), "1");
}

/// A merge that conflicts is an outcome, not an error: git exits non-zero for both a conflict
/// and a genuine failure, so only the repository's own state tells them apart.
#[tokio::test]
async fn a_conflicting_merge_reports_conflicts_rather_than_failing() {
    let repo = conflicting();
    let (runner, loc) = open(&repo).await;

    let outcome = loc
        .merge(&runner, "side", MergeMode::Auto, None)
        .await
        .unwrap();

    assert!(!outcome.completed);
    assert_eq!(outcome.conflicts, vec!["f.txt"]);
    assert_eq!(outcome.state, OpState::Merge);
}

#[tokio::test]
async fn resolves_a_conflict_and_continues() {
    let repo = conflicting();
    let (runner, loc) = open(&repo).await;
    let stopped = loc
        .merge(&runner, "side", MergeMode::Auto, None)
        .await
        .unwrap();
    assert!(!stopped.completed);

    std::fs::write(repo.path().join("f.txt"), "resolved\n").unwrap();
    loc.stage(&runner, &["f.txt"]).await.unwrap();
    let done = loc.op(&runner, OpAction::Continue).await.unwrap();

    assert!(done.completed, "continue should finish the merge");
    assert_eq!(loc.op_state(), OpState::Clean);
    assert_eq!(repo.git(["rev-list", "--count", "--merges", "HEAD"]), "1");
}

#[tokio::test]
async fn aborts_a_conflicting_merge_back_to_the_branch_tip() {
    let repo = conflicting();
    let before = repo.git(["rev-parse", "HEAD"]);
    let (runner, loc) = open(&repo).await;

    loc.merge(&runner, "side", MergeMode::Auto, None)
        .await
        .unwrap();
    let aborted = loc.op(&runner, OpAction::Abort).await.unwrap();

    assert!(aborted.completed);
    assert_eq!(loc.op_state(), OpState::Clean);
    assert_eq!(repo.git(["rev-parse", "HEAD"]), before);
    assert_eq!(
        std::fs::read_to_string(repo.path().join("f.txt")).unwrap(),
        "main\n"
    );
}

#[tokio::test]
async fn a_fast_forward_only_merge_refuses_to_diverge() {
    let repo = conflicting();
    let (runner, loc) = open(&repo).await;

    assert!(
        loc.merge(&runner, "side", MergeMode::FfOnly, None)
            .await
            .is_err()
    );
    assert_eq!(
        loc.op_state(),
        OpState::Clean,
        "a refused merge leaves nothing behind"
    );
}

/// A branch that is only behind, which is what fast-forward is for.
fn behind() -> TestRepo {
    let r = TestRepo::new().write("f.txt", "base\n").commit("base");
    r.git(["checkout", "--quiet", "-b", "side"]);
    let r = r.write("f.txt", "ahead\n").commit("side change");
    r.git(["checkout", "--quiet", "main"]);
    r
}

#[tokio::test]
async fn a_fast_forward_only_merge_moves_the_branch_when_it_can() {
    let repo = behind();
    let (runner, loc) = open(&repo).await;

    let outcome = loc
        .merge(&runner, "side", MergeMode::FfOnly, None)
        .await
        .unwrap();
    assert!(outcome.completed);
    assert_eq!(
        repo.git(["rev-parse", "HEAD"]),
        repo.git(["rev-parse", "side"]),
        "the branch is the other one now"
    );
    assert_eq!(
        repo.git(["rev-list", "--count", "--merges", "HEAD"]),
        "0",
        "and no merge commit was made"
    );
}

#[tokio::test]
async fn a_squash_merge_stages_the_change_without_committing_it() {
    let repo = TestRepo::new().write("f.txt", "base\n").commit("base");
    repo.git(["checkout", "--quiet", "-b", "side"]);
    let repo = repo.write("g.txt", "theirs\n").commit("side change");
    repo.git(["checkout", "--quiet", "main"]);
    let (runner, loc) = open(&repo).await;

    loc.merge(&runner, "side", MergeMode::Squash, None)
        .await
        .unwrap();

    assert_eq!(repo.git(["show", ":g.txt"]), "theirs");
    assert_eq!(
        repo.git(["rev-list", "--count", "HEAD"]),
        "1",
        "nothing was committed"
    );
}

/// Two commits that both touch the line the other branch changed.
///
/// The one scenario a rebase has to get right and the hardest to get right: it stops once per
/// conflicting commit, and continuing lands on the next rather than finishing.
fn two_conflicting_commits() -> TestRepo {
    let r = TestRepo::new().write("f.txt", "base\n").commit("base");
    r.git(["checkout", "--quiet", "-b", "side"]);
    let r = r.write("f.txt", "theirs\n").commit("their change");
    r.git(["checkout", "--quiet", "main"]);
    let r = r.write("f.txt", "ours one\n").commit("our first change");
    r.write("f.txt", "ours two\n").commit("our second change")
}

#[tokio::test]
async fn a_rebase_stops_once_for_every_commit_that_conflicts() {
    let repo = two_conflicting_commits();
    let (runner, loc) = open(&repo).await;

    let first = loc.rebase(&runner, "side", false).await.unwrap();
    assert!(!first.completed);
    assert!(
        first.message.contains("our first change"),
        "it says which commit stopped it: {}",
        first.message
    );

    // The upstream side, which is not what the second commit was written against — so that
    // one cannot apply either. Taking the replayed side would leave the file exactly as the
    // next commit expects, and the rebase would sail through it.
    loc.resolve(&runner, "f.txt", &Resolution::TakeOurs)
        .await
        .unwrap();
    let second = loc.op(&runner, OpAction::Continue).await.unwrap();
    assert!(
        !second.completed,
        "continuing lands on the next conflicting commit, it does not finish"
    );
    assert_eq!(second.state, OpState::Rebase);
    assert_eq!(second.conflicts, vec!["f.txt"]);
    assert!(
        second.message.contains("our second change"),
        "and names that one: {}",
        second.message
    );

    loc.resolve(&runner, "f.txt", &Resolution::TakeTheirs)
        .await
        .unwrap();
    let done = loc.op(&runner, OpAction::Continue).await.unwrap();
    assert!(done.completed);
    assert_eq!(loc.op_state(), OpState::Clean);
    assert_eq!(
        repo.git(["rev-parse", "--abbrev-ref", "HEAD"]),
        "main",
        "and it lands back on the branch it started from"
    );
    // Resolving the first one to the upstream's own content left it with nothing to say, and
    // git drops a commit that comes out empty. The second is replayed on top, which is the
    // part that matters: the rebase ran to the end rather than stopping for good.
    assert_eq!(
        repo.git(["log", "--format=%s", "-2"]),
        "our second change\ntheir change"
    );
}

#[tokio::test]
async fn a_rebase_can_skip_the_commit_that_will_not_apply() {
    let repo = two_conflicting_commits();
    let (runner, loc) = open(&repo).await;

    let stopped = loc.rebase(&runner, "side", false).await.unwrap();
    assert!(!stopped.completed);

    // Dropping the commit rather than resolving it, which is what git's --skip does and what
    // the window offers beside Continue.
    let next = loc.op(&runner, OpAction::Skip).await.unwrap();
    assert!(!next.completed, "the second commit conflicts as well");

    loc.resolve(&runner, "f.txt", &Resolution::TakeTheirs)
        .await
        .unwrap();
    let done = loc.op(&runner, OpAction::Continue).await.unwrap();
    assert!(done.completed);
    assert_eq!(
        repo.git(["log", "--format=%s", "-2"]),
        "our second change\ntheir change",
        "the skipped commit is gone and the other one is not"
    );
}

#[tokio::test]
async fn a_rebase_aborted_mid_conflict_leaves_the_branch_where_it_was() {
    let repo = two_conflicting_commits();
    let (runner, loc) = open(&repo).await;
    let before = repo.git(["rev-parse", "main"]);

    loc.rebase(&runner, "side", false).await.unwrap();
    loc.op(&runner, OpAction::Abort).await.unwrap();

    assert_eq!(loc.op_state(), OpState::Clean);
    assert_eq!(repo.git(["rev-parse", "main"]), before);
    assert_eq!(repo.git(["rev-parse", "--abbrev-ref", "HEAD"]), "main");
}

#[tokio::test]
async fn a_conflicting_merge_resolved_with_both_sides_records_the_merge() {
    let repo = conflicting();
    let (runner, loc) = open(&repo).await;

    let stopped = loc
        .merge(&runner, "side", MergeMode::Auto, None)
        .await
        .unwrap();
    assert!(!stopped.completed);

    // Both lines kept, in the order the window would have taken them: this is the resolution
    // that neither `--ours` nor `--theirs` can express.
    loc.resolve(
        &runner,
        "f.txt",
        &Resolution::Content("side\nmain\n".into()),
    )
    .await
    .unwrap();
    let done = loc.op(&runner, OpAction::Continue).await.unwrap();
    assert!(done.completed);

    assert_eq!(
        repo.git(["show", "HEAD:f.txt"]),
        "side\nmain",
        "the file is what was written, not one side of it"
    );
    assert_eq!(
        repo.git(["rev-list", "--parents", "-n", "1", "HEAD"])
            .split_whitespace()
            .count(),
        3,
        "and the commit has both parents"
    );
}

#[tokio::test]
async fn rebases_and_stops_on_conflict() {
    let repo = conflicting();
    let (runner, loc) = open(&repo).await;

    let outcome = loc.rebase(&runner, "side", false).await.unwrap();
    assert!(!outcome.completed);
    assert_eq!(outcome.state, OpState::Rebase);
    assert_eq!(outcome.conflicts, vec!["f.txt"]);

    let aborted = loc.op(&runner, OpAction::Abort).await.unwrap();
    assert!(aborted.completed);
    assert_eq!(loc.op_state(), OpState::Clean);
}

#[tokio::test]
async fn cherry_picks_and_reverts() {
    let repo = TestRepo::new().write("a.txt", "base\n").commit("base");
    repo.git(["checkout", "--quiet", "-b", "side"]);
    let repo = repo.write("b.txt", "from side\n").commit("side work");
    let picked = repo.git(["rev-parse", "HEAD"]);
    repo.git(["checkout", "--quiet", "main"]);

    let (runner, loc) = open(&repo).await;
    let out = loc
        .cherry_pick(&runner, &[&picked], true, None)
        .await
        .unwrap();
    assert!(out.completed);
    assert!(repo.path().join("b.txt").exists());

    let out = loc.revert(&runner, &["HEAD"], None).await.unwrap();
    assert!(out.completed);
    assert!(!repo.path().join("b.txt").exists(), "the revert undid it");
}

#[tokio::test]
async fn reverts_a_merge_when_told_which_side_to_keep() {
    // A merge has two sides, so undoing it means keeping one. Asked without a mainline git
    // refuses with "is a merge but no -m option was given", which is a sentence about its
    // command line rather than about the repository — and that is what the window handed over.
    let repo = TestRepo::new().write("a.txt", "base\n").commit("base");
    repo.git(["checkout", "--quiet", "-b", "side"]);
    let repo = repo.write("b.txt", "from side\n").commit("side work");
    repo.git(["checkout", "--quiet", "main"]);
    repo.git(["merge", "--quiet", "--no-ff", "-m", "merge side", "side"]);
    assert!(repo.path().join("b.txt").exists());

    let (runner, loc) = open(&repo).await;
    assert!(
        loc.revert(&runner, &["HEAD"], None).await.is_err(),
        "git refuses a merge with no mainline named"
    );

    let out = loc.revert(&runner, &["HEAD"], Some(1)).await.unwrap();
    assert!(out.completed);
    assert!(
        !repo.path().join("b.txt").exists(),
        "keeping the first parent undoes what the other side brought"
    );
}

#[tokio::test]
async fn cherry_picks_a_merge_when_told_which_side_to_keep() {
    let repo = TestRepo::new().write("a.txt", "base\n").commit("base");
    let base = repo.git(["rev-parse", "HEAD"]);
    repo.git(["checkout", "--quiet", "-b", "side"]);
    let repo = repo.write("b.txt", "from side\n").commit("side work");
    repo.git(["checkout", "--quiet", "main"]);
    let repo = repo.write("c.txt", "on main\n").commit("main work");
    repo.git(["merge", "--quiet", "--no-ff", "-m", "merge side", "side"]);
    let merge = repo.git(["rev-parse", "HEAD"]);
    repo.git(["checkout", "--quiet", "-b", "elsewhere", &base]);

    let (runner, loc) = open(&repo).await;
    assert!(
        loc.cherry_pick(&runner, &[&merge], true, None)
            .await
            .is_err(),
        "git refuses a merge with no mainline named"
    );

    let out = loc
        .cherry_pick(&runner, &[&merge], true, Some(1))
        .await
        .unwrap();
    assert!(out.completed);
    assert!(repo.path().join("b.txt").exists());
}

#[tokio::test]
async fn cherry_picks_without_committing_when_asked() {
    // For someone who wants to change it, split it, or fold it into something else before
    // anything is recorded. The changes are in the index; HEAD has not moved.
    let repo = TestRepo::new().write("a.txt", "base\n").commit("base");
    repo.git(["checkout", "--quiet", "-b", "side"]);
    let repo = repo.write("b.txt", "from side\n").commit("side work");
    let picked = repo.git(["rev-parse", "HEAD"]);
    repo.git(["checkout", "--quiet", "main"]);
    let before = repo.git(["rev-parse", "HEAD"]);

    let (runner, loc) = open(&repo).await;
    let out = loc
        .cherry_pick(&runner, &[&picked], false, None)
        .await
        .unwrap();

    assert!(out.completed);
    assert!(repo.path().join("b.txt").exists(), "the change is on disk");
    assert_eq!(repo.git(["rev-parse", "HEAD"]), before, "and not committed");
    assert!(
        repo.git(["status", "--porcelain"]).contains("A  b.txt"),
        "and staged: {}",
        repo.git(["status", "--porcelain"])
    );
}

#[tokio::test]
async fn a_conflicting_cherry_pick_stops_and_says_which_file() {
    // The case the window has to show a merge tool for. git leaves the pick in progress, so it
    // can be resolved and continued exactly like a merge.
    let repo = TestRepo::new().write("a.txt", "base\n").commit("base");
    repo.git(["checkout", "--quiet", "-b", "side"]);
    let repo = repo.write("a.txt", "from side\n").commit("side work");
    let picked = repo.git(["rev-parse", "HEAD"]);
    repo.git(["checkout", "--quiet", "main"]);
    let repo = repo.write("a.txt", "from main\n").commit("main work");

    let (runner, loc) = open(&repo).await;
    let out = loc
        .cherry_pick(&runner, &[&picked], true, None)
        .await
        .unwrap();

    assert!(!out.completed, "it stopped");
    assert_eq!(out.conflicts, vec!["a.txt".to_owned()]);
    assert_eq!(
        loc.op_state(),
        OpState::CherryPick,
        "and git knows what is in progress, so it can be continued or abandoned"
    );
}

#[tokio::test]
async fn a_conflicting_cherry_pick_can_be_resolved_and_continued() {
    let repo = TestRepo::new().write("a.txt", "base\n").commit("base");
    repo.git(["checkout", "--quiet", "-b", "side"]);
    let repo = repo.write("a.txt", "from side\n").commit("side work");
    let picked = repo.git(["rev-parse", "HEAD"]);
    repo.git(["checkout", "--quiet", "main"]);
    let repo = repo.write("a.txt", "from main\n").commit("main work");

    let (runner, loc) = open(&repo).await;
    loc.cherry_pick(&runner, &[&picked], true, None)
        .await
        .unwrap();

    std::fs::write(repo.path().join("a.txt"), "settled\n").unwrap();
    loc.stage(&runner, &["a.txt"]).await.unwrap();
    let out = loc.op(&runner, OpAction::Continue).await.unwrap();

    assert!(out.completed);
    assert_eq!(loc.op_state(), OpState::Clean);
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "settled\n"
    );
}

#[tokio::test]
async fn a_conflicting_cherry_pick_can_be_abandoned() {
    let repo = TestRepo::new().write("a.txt", "base\n").commit("base");
    repo.git(["checkout", "--quiet", "-b", "side"]);
    let repo = repo.write("a.txt", "from side\n").commit("side work");
    let picked = repo.git(["rev-parse", "HEAD"]);
    repo.git(["checkout", "--quiet", "main"]);
    let repo = repo.write("a.txt", "from main\n").commit("main work");
    let before = repo.git(["rev-parse", "HEAD"]);

    let (runner, loc) = open(&repo).await;
    loc.cherry_pick(&runner, &[&picked], true, None)
        .await
        .unwrap();
    loc.op(&runner, OpAction::Abort).await.unwrap();

    assert_eq!(repo.git(["rev-parse", "HEAD"]), before);
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "from main\n",
        "the working copy is as it was"
    );
    assert_eq!(loc.op_state(), OpState::Clean);
}

#[tokio::test]
async fn continuing_with_nothing_in_progress_is_an_error() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let (runner, loc) = open(&repo).await;

    let err = loc.op(&runner, OpAction::Continue).await.unwrap_err();
    assert_eq!(err.code(), "refused");
}
