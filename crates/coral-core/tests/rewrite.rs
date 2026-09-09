//! The commit menu's history edits: drop, reword, and move one commit past its neighbour.
//!
//! Each of these rewrites the branch, so what the tests check is not only that the operation
//! reported success but that the resulting history is the one asked for and nothing else moved.

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::sequence::{Rewrite, Step, Todo, TodoItem, apply_rewrite};
use coral_core::testutil::TestRepo;

/// Four commits, oldest first: base, a, b, c.
fn stack() -> TestRepo {
    TestRepo::new()
        .write("base.txt", "base\n")
        .commit("base")
        .write("a.txt", "a\n")
        .commit("commit a")
        .write("b.txt", "b\n")
        .commit("commit b")
        .write("c.txt", "c\n")
        .commit("commit c")
}

fn coral_binary() -> std::path::PathBuf {
    let mut at = std::env::current_exe().expect("test binary path");
    at.pop();
    if at.ends_with("deps") {
        at.pop();
    }
    let binary = at.join(if cfg!(windows) { "coral.exe" } else { "coral" });
    assert!(
        binary.exists(),
        "the CLI must be built for these: {}",
        binary.display()
    );
    binary
}

/// Newest first, as the graph lists them.
fn summaries(repo: &TestRepo) -> Vec<String> {
    repo.git(["log", "--format=%s"])
        .lines()
        .map(str::to_owned)
        .collect()
}

fn run<F, T>(f: F) -> T
where
    F: std::future::Future<Output = T>,
{
    tokio::runtime::Runtime::new().unwrap().block_on(f)
}

async fn located(repo: &TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

fn item(oid: &str, summary: &str) -> TodoItem {
    TodoItem {
        step: Step::Pick,
        oid: oid.to_owned(),
        summary: summary.into(),
        message: None,
    }
}

#[test]
fn dropping_a_commit_removes_it_and_keeps_the_rest() {
    let repo = stack();
    let b = repo.git(["rev-parse", "HEAD~1"]);
    run(async {
        let (runner, loc) = located(&repo).await;
        let outcome = loc
            .rewrite_commit(&runner, &b, &Rewrite::Drop, &coral_binary())
            .await
            .unwrap();
        assert!(outcome.completed, "{outcome:?}");
    });

    assert_eq!(summaries(&repo), ["commit c", "commit a", "base"]);
    // Its file goes with it; a drop that left the content behind would be a revert, not a drop.
    assert!(!repo.path().join("b.txt").exists());
    assert!(repo.path().join("c.txt").exists());
}

#[test]
fn rewording_replaces_one_message_and_leaves_the_tree_alone() {
    let repo = stack();
    let b = repo.git(["rev-parse", "HEAD~1"]);
    let tree_before = repo.git(["rev-parse", "HEAD^{tree}"]);
    run(async {
        let (runner, loc) = located(&repo).await;
        let outcome = loc
            .rewrite_commit(
                &runner,
                &b,
                &Rewrite::Reword("commit b, said better".to_owned()),
                &coral_binary(),
            )
            .await
            .unwrap();
        assert!(outcome.completed, "{outcome:?}");
    });

    assert_eq!(
        summaries(&repo),
        ["commit c", "commit b, said better", "commit a", "base"]
    );
    assert_eq!(
        repo.git(["rev-parse", "HEAD^{tree}"]),
        tree_before,
        "a reword must not change what the commit contains"
    );
}

#[test]
fn moving_a_commit_towards_head_swaps_it_with_its_child() {
    let repo = stack();
    let b = repo.git(["rev-parse", "HEAD~1"]);
    run(async {
        let (runner, loc) = located(&repo).await;
        let outcome = loc
            .rewrite_commit(&runner, &b, &Rewrite::MoveNewer, &coral_binary())
            .await
            .unwrap();
        assert!(outcome.completed, "{outcome:?}");
    });

    assert_eq!(
        summaries(&repo),
        ["commit b", "commit c", "commit a", "base"]
    );
    // Both files survive the reordering; only the order of the commits changed.
    assert!(repo.path().join("b.txt").exists());
    assert!(repo.path().join("c.txt").exists());
}

#[test]
fn moving_a_commit_away_from_head_swaps_it_with_its_parent() {
    let repo = stack();
    let b = repo.git(["rev-parse", "HEAD~1"]);
    run(async {
        let (runner, loc) = located(&repo).await;
        let outcome = loc
            .rewrite_commit(&runner, &b, &Rewrite::MoveOlder, &coral_binary())
            .await
            .unwrap();
        assert!(outcome.completed, "{outcome:?}");
    });

    assert_eq!(
        summaries(&repo),
        ["commit c", "commit a", "commit b", "base"]
    );
}

#[test]
fn a_commit_that_is_not_on_the_branch_is_refused() {
    // Rewriting it would either do nothing or quietly drag unrelated work onto this branch.
    let repo = stack();
    repo.git(["checkout", "--quiet", "-b", "side", "HEAD~2"]);
    let elsewhere = repo.git(["rev-parse", "main"]);
    repo.git(["checkout", "--quiet", "main"]);
    repo.git(["checkout", "--quiet", "side"]);

    let error = run(async {
        let (runner, loc) = located(&repo).await;
        loc.rewrite_commit(&runner, &elsewhere, &Rewrite::Drop, &coral_binary())
            .await
            .unwrap_err()
    });
    assert_eq!(error.code(), "refused", "{error}");
    assert!(
        error.to_string().contains("not on the current branch"),
        "{error}"
    );
}

#[test]
fn a_range_containing_a_merge_is_refused_rather_than_flattened() {
    // The todo list is built without merges, so replaying such a range would silently rewrite
    // the history into a straight line.
    let repo = TestRepo::new()
        .write("base.txt", "base\n")
        .commit("base")
        .write("a.txt", "a\n")
        .commit("commit a");
    repo.git(["checkout", "--quiet", "-b", "side", "HEAD~1"]);
    let repo = repo.write("side.txt", "side\n").commit("commit side");
    repo.git(["checkout", "--quiet", "main"]);
    repo.git(["merge", "--quiet", "--no-ff", "-m", "merge side", "side"]);
    let a = repo.git(["rev-parse", "main~1"]);

    let error = run(async {
        let (runner, loc) = located(&repo).await;
        loc.rewrite_commit(&runner, &a, &Rewrite::Drop, &coral_binary())
            .await
            .unwrap_err()
    });
    assert_eq!(error.code(), "refused", "{error}");
    assert!(error.to_string().contains("merge"), "{error}");
}

#[test]
fn the_newest_commit_cannot_be_moved_any_newer() {
    let mut todo = Todo {
        items: vec![item("aaa", "older"), item("bbb", "newest")],
    };
    let error = apply_rewrite(&mut todo, 1, &Rewrite::MoveNewer).unwrap_err();
    assert!(error.to_string().contains("nothing above it"), "{error}");
    // And the list is left exactly as it was, so a refused edit cannot half-apply.
    assert_eq!(todo.items[0].oid, "aaa");
    assert_eq!(todo.items[1].oid, "bbb");
}

#[test]
fn the_oldest_replayed_commit_cannot_be_moved_any_older() {
    let mut todo = Todo {
        items: vec![item("aaa", "oldest"), item("bbb", "newer")],
    };
    let error = apply_rewrite(&mut todo, 0, &Rewrite::MoveOlder).unwrap_err();
    assert!(error.to_string().contains("nothing below it"), "{error}");
}

#[test]
fn a_reword_carries_its_message_on_the_item_it_belongs_to() {
    let mut todo = Todo {
        items: vec![item("aaa", "one"), item("bbb", "two")],
    };
    apply_rewrite(&mut todo, 1, &Rewrite::Reword("said again".to_owned())).unwrap();
    assert_eq!(todo.items[1].step, Step::Reword);
    assert_eq!(todo.items[1].message.as_deref(), Some("said again"));
    assert_eq!(
        todo.items[0].step,
        Step::Pick,
        "the other item is untouched"
    );
    assert_eq!(todo.items[0].message, None);
}
