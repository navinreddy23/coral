//! Sparse checkout in cone mode: which folders the working tree holds, changed and read back
//! against what is actually on disk.

use coral_core::create::Cloned;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

fn three_folders() -> TestRepo {
    TestRepo::new()
        .write("top.txt", "top\n")
        .write("a/one.txt", "1\n")
        .write("b/two.txt", "2\n")
        .write("c/deep/three.txt", "3\n")
        .commit("base")
}

#[tokio::test]
async fn a_cone_keeps_the_folders_named_and_the_files_at_the_top() {
    let repo = three_folders();
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    assert!(!loc.sparse(&runner).await.unwrap().enabled);

    loc.set_sparse(&runner, &["c/deep".to_owned()])
        .await
        .unwrap();
    let sparse = loc.sparse(&runner).await.unwrap();
    assert!(sparse.enabled && sparse.cone);
    assert_eq!(sparse.folders, ["c/deep"]);
    assert!(repo.path().join("top.txt").exists());
    assert!(repo.path().join("c/deep/three.txt").exists());
    assert!(!repo.path().join("a").exists());
    assert!(!repo.path().join("b").exists());
    // What is left out is not a change: the status stays clean.
    assert_eq!(loc.status(&runner).await.unwrap().entries, []);

    loc.disable_sparse(&runner).await.unwrap();
    assert!(!loc.sparse(&runner).await.unwrap().enabled);
    assert!(repo.path().join("a/one.txt").exists());
}

#[tokio::test]
async fn the_picker_reads_one_level_of_folders_from_the_tree_not_the_disk() {
    let repo = three_folders();
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    loc.set_sparse(&runner, &[]).await.unwrap();

    assert_eq!(
        loc.folders(&runner, "HEAD", None).await.unwrap(),
        ["a", "b", "c"]
    );
    assert_eq!(
        loc.folders(&runner, "HEAD", Some("c")).await.unwrap(),
        ["c/deep"]
    );
    assert_eq!(
        loc.folders(&runner, "HEAD", Some("a")).await.unwrap(),
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn a_sparse_clone_starts_with_the_files_at_the_top_alone() {
    let source = three_folders();
    let into = tempfile::tempdir().unwrap();
    let runner = GitRunner::discover().await.unwrap();
    let made = coral_core::create::clone(
        &runner,
        &Cloned {
            url: format!("file://{}", source.path().display()),
            parent: into.path().to_path_buf(),
            name: Some("sparse".to_owned()),
            ssh_key: None,
            depth: None,
            blobless: false,
            sparse: true,
        },
        |_| {},
    )
    .await
    .unwrap();
    let at = made.at;
    assert!(at.join("top.txt").exists());
    assert!(!at.join("a").exists());
    let loc = RepoLocation::discover(&runner, &at).await.unwrap();
    let sparse = loc.sparse(&runner).await.unwrap();
    assert!(sparse.enabled);
    assert_eq!(sparse.folders, Vec::<String>::new());
}
