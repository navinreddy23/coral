//! The staging commands. Each returns the resulting status rather than nothing, so the panel
//! cannot drift from the repository; these check that the returned status is the new one.

use coral_app_lib as app;
use coral_core::testutil::TestRepo;

#[tokio::test(flavor = "multi_thread")]
async fn status_separates_staged_unstaged_and_untracked() {
    let repo = TestRepo::new().write("tracked.txt", "1\n").commit("base");
    let repo = repo
        .write("tracked.txt", "changed\n")
        .write("staged.txt", "s\n");
    repo.git(["add", "staged.txt"]);
    std::fs::write(repo.path().join("untracked.txt"), "u\n").unwrap();

    let status = app::repo_status(repo.path().display().to_string())
        .await
        .unwrap();
    let find = |p: &str| status.entries.iter().find(|e| e.path == p).unwrap();

    assert_eq!(status.entries.len(), 3);
    assert!(find("staged.txt").is_staged());
    assert!(!find("tracked.txt").is_staged());
    assert_eq!(
        find("untracked.txt").index,
        coral_core::status::Change::Untracked
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn staging_returns_the_status_after_the_change() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let repo = repo.write("a.txt", "2\n");
    let path = repo.path().display().to_string();

    let after = app::stage_paths(path.clone(), vec!["a.txt".into()], true)
        .await
        .unwrap();
    let entry = after.entries.iter().find(|e| e.path == "a.txt").unwrap();
    assert!(
        entry.is_staged(),
        "the returned status already reflects the staging"
    );

    let after = app::stage_paths(path, vec!["a.txt".into()], false)
        .await
        .unwrap();
    let entry = after.entries.iter().find(|e| e.path == "a.txt").unwrap();
    assert!(!entry.is_staged());
}

#[tokio::test(flavor = "multi_thread")]
async fn committing_clears_the_staged_changes() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let repo = repo.write("a.txt", "2\n");
    let path = repo.path().display().to_string();

    app::stage_paths(path.clone(), vec!["a.txt".into()], true)
        .await
        .unwrap();
    let after = app::commit_staged(path, "the message".into(), false)
        .await
        .unwrap();

    assert!(after.is_clean(), "nothing left once it is committed");
    assert_eq!(repo.git(["log", "-1", "--format=%s"]), "the message");
}

#[tokio::test(flavor = "multi_thread")]
async fn amending_replaces_the_previous_commit() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("original");
    let repo = repo.write("a.txt", "2\n");
    let path = repo.path().display().to_string();

    app::stage_paths(path.clone(), vec!["a.txt".into()], true)
        .await
        .unwrap();
    app::commit_staged(path, "reworded".into(), true)
        .await
        .unwrap();

    assert_eq!(
        repo.git(["rev-list", "--count", "HEAD"]),
        "1",
        "amend adds no commit"
    );
    assert_eq!(repo.git(["log", "-1", "--format=%s"]), "reworded");
}

/// A path that is not there must surface git's refusal rather than silently doing nothing.
#[tokio::test(flavor = "multi_thread")]
async fn staging_a_path_that_does_not_exist_is_an_error() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let path = repo.path().display().to_string();

    assert!(
        app::stage_paths(path, vec!["nope.txt".into()], true)
            .await
            .is_err()
    );
}
