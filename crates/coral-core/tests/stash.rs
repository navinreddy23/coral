//! Listing the stash stack against real git.
//!
//! The parser's own tests cover the shapes; these cover the agreement with git, which is where
//! a format string quietly stops meaning what it did.

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

async fn open(repo: &TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

#[tokio::test]
async fn an_untouched_repository_has_no_stashes() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let (runner, loc) = open(&repo).await;
    let stashes = loc.stashes(&runner).await.unwrap();
    assert!(stashes.is_empty(), "{stashes:?}");
}

#[tokio::test]
async fn lists_the_stack_newest_first_with_positions_that_work() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let (runner, loc) = open(&repo).await;

    let repo = repo.write("a.txt", "first stash\n");
    repo.git(["stash", "push", "--quiet", "-m", "the older one"]);
    let repo = repo.write("a.txt", "second stash\n");
    repo.git(["stash", "push", "--quiet", "-m", "the newer one"]);

    let stack = loc.stashes(&runner).await.unwrap();
    assert_eq!(stack.len(), 2);
    assert_eq!(stack[0].index, 0);
    assert_eq!(stack[0].message, "the newer one");
    assert_eq!(stack[1].index, 1);
    assert_eq!(stack[1].message, "the older one");

    // The position is the thing apply and drop are given, so it has to name what it looks like.
    loc.stash_apply(&runner, stack[1].index, false)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "first stash\n"
    );
}

#[tokio::test]
async fn two_stashes_from_the_same_commit_are_told_apart() {
    // What the window could not do: both carry the identical subject, so the list showed two
    // rows with one name and no way to say which was which.
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let (runner, loc) = open(&repo).await;

    let repo = repo.write("a.txt", "one\n");
    repo.git(["stash", "push", "--quiet"]);
    let repo = repo.write("a.txt", "two\n");
    repo.git(["stash", "push", "--quiet"]);

    let stack = loc.stashes(&runner).await.unwrap();
    assert_eq!(
        stack[0].message, stack[1].message,
        "git says the same thing"
    );
    assert_ne!(stack[0].name(), stack[1].name(), "and Coral does not");
    assert!(stack[0].name().starts_with("master@") || stack[0].name().starts_with("main@"));
}

#[tokio::test]
async fn a_named_stash_keeps_the_name_and_the_branch() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    repo.git(["checkout", "--quiet", "-b", "topic"]);
    let repo = repo.write("a.txt", "2\n");
    repo.git(["stash", "push", "--quiet", "-m", "half a refactor"]);

    let (runner, loc) = open(&repo).await;
    let stack = loc.stashes(&runner).await.unwrap();
    assert_eq!(stack[0].branch.as_deref(), Some("topic"));
    assert_eq!(stack[0].message, "half a refactor");
    assert!(stack[0].name().starts_with("topic@"));
}

#[tokio::test]
async fn stashing_before_the_first_commit_says_why_it_cannot() {
    let repo = TestRepo::new().write("a.txt", "1\n");
    let (runner, loc) = open(&repo).await;
    let err = loc
        .stash_push(
            &runner,
            None,
            &coral_core::ops::StashScope::Everything {
                include_untracked: true,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "refused");
    assert!(err.to_string().contains("first commit"), "{err}");
}

#[tokio::test]
async fn a_stash_becomes_a_branch_where_it_was_taken_and_leaves_the_stack() {
    let repo = TestRepo::new().write("a.txt", "base\n").commit("base");
    let base = repo.git(["rev-parse", "HEAD"]).trim().to_owned();
    let repo = repo.write("a.txt", "half done\n");
    repo.git(["stash", "push", "--quiet", "-m", "half done"]);
    let repo = repo.write("a.txt", "moved on\n").commit("moved on");
    let (runner, loc) = open(&repo).await;

    let out = loc.stash_branch(&runner, "half-done", 0).await.unwrap();
    assert!(out.completed, "{out:?}");
    assert_eq!(repo.git(["branch", "--show-current"]).trim(), "half-done");
    assert_eq!(
        repo.git(["rev-parse", "HEAD"]).trim(),
        base,
        "made where the stash was taken"
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "half done\n"
    );
    assert!(
        repo.git(["stash", "list"]).trim().is_empty(),
        "the entry is dropped"
    );

    let taken = loc.stash_branch(&runner, "half-done", 0).await;
    assert!(taken.is_err(), "nothing left to make a branch from");
}
