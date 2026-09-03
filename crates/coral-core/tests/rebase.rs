//! Interactive rebase driven by a prepared todo list, with no editor on the path.

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::sequence::{Step, Todo};
use coral_core::testutil::TestRepo;

/// Three commits on top of a base, oldest first: a, b, c.
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
    // The test binary sits beside the CLI in the same profile directory.
    let mut at = std::env::current_exe().expect("test binary path");
    at.pop();
    if at.ends_with("deps") {
        at.pop();
    }
    let binary = at.join("coral");
    assert!(
        binary.exists(),
        "the CLI must be built for these: {}",
        binary.display()
    );
    binary
}

fn summaries(repo: &TestRepo) -> Vec<String> {
    repo.git(["log", "--format=%s"])
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_todo_lists_the_range_oldest_first() {
    let repo = stack();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let todo = loc.rebase_todo(&runner, "HEAD~3").await.unwrap();

        // Replay order, which is the reverse of how the graph lists them.
        let order: Vec<_> = todo.items.iter().map(|i| i.summary.to_string()).collect();
        assert_eq!(order, ["commit a", "commit b", "commit c"]);
        assert!(todo.items.iter().all(|i| i.step == Step::Pick));
        assert!(todo.items.iter().all(|i| i.oid.len() == 40));
    });
}

#[test]
fn dropping_a_commit_removes_it_and_its_file() {
    let repo = stack();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let mut todo = loc.rebase_todo(&runner, "HEAD~3").await.unwrap();
        todo.items[1].step = Step::Drop;

        let out = loc
            .rebase_interactive(&runner, "HEAD~3", &todo, &coral_binary())
            .await
            .unwrap();
        assert!(
            out.completed,
            "no conflict: the three touch different files. git said: {}",
            out.message
        );
    });

    assert_eq!(summaries(&repo), ["commit c", "commit a", "base"]);
    assert!(!repo.path().join("b.txt").exists());
}

#[test]
fn reordering_replays_in_the_new_order() {
    let repo = stack();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let mut todo = loc.rebase_todo(&runner, "HEAD~3").await.unwrap();
        todo.reorder(2, 0).unwrap();

        let out = loc
            .rebase_interactive(&runner, "HEAD~3", &todo, &coral_binary())
            .await
            .unwrap();
        assert!(out.completed);
    });

    assert_eq!(
        summaries(&repo),
        ["commit b", "commit a", "commit c", "base"]
    );
}

#[test]
fn a_fixup_folds_a_commit_into_the_one_before_it() {
    let repo = stack();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let mut todo = loc.rebase_todo(&runner, "HEAD~3").await.unwrap();
        todo.items[2].step = Step::Fixup;

        // A fixup opens an editor on the combined message that nobody is there to answer; the
        // stubbed core.editor is what keeps this from hanging.
        let out = loc
            .rebase_interactive(&runner, "HEAD~3", &todo, &coral_binary())
            .await
            .unwrap();
        assert!(out.completed);
    });

    assert_eq!(summaries(&repo), ["commit b", "commit a", "base"]);
    // The folded commit's work is still there, only its message is gone.
    assert!(repo.path().join("c.txt").exists());
}

#[test]
fn a_squash_keeps_both_messages() {
    let repo = stack();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let mut todo = loc.rebase_todo(&runner, "HEAD~3").await.unwrap();
        todo.items[2].step = Step::Squash;
        let out = loc
            .rebase_interactive(&runner, "HEAD~3", &todo, &coral_binary())
            .await
            .unwrap();
        assert!(out.completed);
    });

    let body = repo.git(["log", "-1", "--format=%B"]);
    assert!(body.contains("commit b"), "kept the first message: {body}");
    assert!(body.contains("commit c"), "kept the second: {body}");
}

#[test]
fn a_list_that_starts_with_a_squash_is_refused_before_anything_moves() {
    // git would fail partway through, leaving a rebase to abort; refusing first leaves the
    // branch exactly where it was.
    let repo = stack();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let before = repo.git(["rev-parse", "HEAD"]);
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let mut todo = loc.rebase_todo(&runner, "HEAD~3").await.unwrap();
        todo.items[0].step = Step::Squash;
        assert!(
            loc.rebase_interactive(&runner, "HEAD~3", &todo, &coral_binary())
                .await
                .is_err()
        );
    });
    assert_eq!(repo.git(["rev-parse", "HEAD"]), before);
    assert_eq!(repo.git(["status", "--porcelain"]).trim(), "");
}

#[test]
fn the_todo_file_is_not_left_behind() {
    let repo = stack();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let todo: Todo = loc.rebase_todo(&runner, "HEAD~3").await.unwrap();
        loc.rebase_interactive(&runner, "HEAD~3", &todo, &coral_binary())
            .await
            .unwrap();
        // Anything reading the git dir would take a leftover as a rebase in progress.
        assert!(!loc.git_path("coral-rebase-todo").exists());
    });
}
