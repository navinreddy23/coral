//! Which path a tab is opened for.
//!
//! A file chooser answers with whatever directory was showing, which is often somewhere inside
//! the repository. Every tab, and every entry on the start page, is named for this path.

use coral_app_lib::tabs::root_of;
use coral_core::testutil::TestRepo;

#[tokio::test(flavor = "multi_thread")]
async fn a_path_inside_a_repository_resolves_to_its_root() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let deep = repo.path().join("nested").join("deep");
    std::fs::create_dir_all(&deep).unwrap();

    let root = root_of(&deep.display().to_string()).await.unwrap();
    assert_eq!(
        std::fs::canonicalize(root).unwrap(),
        std::fs::canonicalize(repo.path()).unwrap(),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bare_repository_is_its_own_root() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let bare = repo.path().join("bare.git");
    repo.git(["clone", "--quiet", "--bare", ".", bare.to_str().unwrap()]);

    let root = root_of(&bare.display().to_string()).await.unwrap();
    assert_eq!(
        std::fs::canonicalize(root).unwrap(),
        std::fs::canonicalize(&bare).unwrap(),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_directory_that_is_not_in_a_repository_has_no_root() {
    let plain = tempfile::tempdir().unwrap();
    assert!(root_of(&plain.path().display().to_string()).await.is_none());
}
