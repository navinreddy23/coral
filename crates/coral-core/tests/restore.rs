//! Discarding changes: putting files back to HEAD, and removing files git has never seen.
//!
//! Every case checks the resulting content on disk and in the index, because this is the one
//! operation in the client that destroys work — passing for the wrong reason here means a user
//! loses something.

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

async fn open(repo: &TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

#[tokio::test]
async fn puts_a_modified_file_back_in_both_the_index_and_the_worktree() {
    let repo = TestRepo::new().write("a.txt", "one\n").commit("base");
    let repo = repo.write("a.txt", "staged\n");
    let (runner, loc) = open(&repo).await;
    loc.stage(&runner, &["a.txt"]).await.unwrap();
    let repo = repo.write("a.txt", "staged and edited\n");

    loc.restore_from_head(&runner, &["a.txt"]).await.unwrap();

    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "one\n",
        "the worktree went back to HEAD"
    );
    assert_eq!(
        repo.git(["status", "--porcelain"]).trim(),
        "",
        "and so did the index"
    );
}

#[tokio::test]
async fn removes_a_file_that_was_staged_as_new() {
    // It is not in HEAD, so "back to HEAD" means gone. Unstaging alone would leave it on disk
    // as an untracked file, which is not what discarding it asked for.
    let repo = TestRepo::new().write("a.txt", "one\n").commit("base");
    let repo = repo.write("new.txt", "fresh\n");
    let (runner, loc) = open(&repo).await;
    loc.stage(&runner, &["new.txt"]).await.unwrap();

    loc.restore_from_head(&runner, &["new.txt"]).await.unwrap();

    assert!(!repo.path().join("new.txt").exists());
    assert_eq!(repo.git(["status", "--porcelain"]).trim(), "");
}

#[tokio::test]
async fn leaves_every_other_file_alone() {
    let repo = TestRepo::new()
        .write("a.txt", "one\n")
        .write("b.txt", "one\n")
        .commit("base");
    let repo = repo.write("a.txt", "edited\n").write("b.txt", "edited\n");
    let (runner, loc) = open(&repo).await;

    loc.restore_from_head(&runner, &["a.txt"]).await.unwrap();

    assert_eq!(
        std::fs::read_to_string(repo.path().join("b.txt")).unwrap(),
        "edited\n",
        "a file that was not named kept its changes"
    );
}

#[tokio::test]
async fn an_empty_list_is_not_an_instruction_to_discard_everything() {
    // The interface can hand this an empty selection, and `git restore --` with no pathspec
    // would be a very expensive way to find that out.
    let repo = TestRepo::new().write("a.txt", "one\n").commit("base");
    let repo = repo.write("a.txt", "edited\n").write("loose.txt", "new\n");
    let (runner, loc) = open(&repo).await;

    loc.restore_from_head(&runner, &[]).await.unwrap();
    loc.remove_untracked(&runner, &[]).await.unwrap();

    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "edited\n"
    );
    assert!(repo.path().join("loose.txt").exists());
}

#[tokio::test]
async fn removes_untracked_files_and_the_directories_holding_them() {
    let repo = TestRepo::new().write("a.txt", "one\n").commit("base");
    let repo = repo
        .write("loose.txt", "new\n")
        .write("fresh/inside.txt", "new\n");
    let (runner, loc) = open(&repo).await;

    loc.remove_untracked(&runner, &["loose.txt", "fresh"])
        .await
        .unwrap();

    assert!(!repo.path().join("loose.txt").exists());
    assert!(!repo.path().join("fresh").exists());
    assert_eq!(repo.git(["status", "--porcelain"]).trim(), "");
}

#[tokio::test]
async fn never_removes_an_ignored_file() {
    // Ignored paths are build output, caches and editor state. Nobody asked about them, and a
    // client that deletes a `target/` directory because someone discarded a typo has done
    // something far larger than it was told to.
    let repo = TestRepo::new()
        .write("a.txt", "one\n")
        .write(".gitignore", "build/\n")
        .commit("base");
    let repo = repo.write("build/output.bin", "artefact\n");
    let (runner, loc) = open(&repo).await;

    loc.remove_untracked(&runner, &["build"]).await.unwrap();

    assert!(
        repo.path().join("build/output.bin").exists(),
        "an ignored file survived being named directly"
    );
}

#[tokio::test]
async fn refuses_a_path_git_does_not_know_rather_than_half_finishing() {
    // git rejects the whole invocation for one unknown pathspec, so the caller has to keep
    // untracked paths out of this list. The test states that, because the alternative — a
    // silent skip — would leave the user believing the discard had happened.
    let repo = TestRepo::new().write("a.txt", "one\n").commit("base");
    let repo = repo.write("a.txt", "edited\n").write("loose.txt", "new\n");
    let (runner, loc) = open(&repo).await;

    let refused = loc
        .restore_from_head(&runner, &["a.txt", "loose.txt"])
        .await;

    assert!(refused.is_err());
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "edited\n",
        "and nothing was discarded"
    );
}
