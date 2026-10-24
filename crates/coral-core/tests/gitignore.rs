//! Ignoring a file from the window: git must then leave out that file, and only that file.

use coral_core::gitignore::exact;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

async fn open(repo: &TestRepo) -> RepoLocation {
    let runner = GitRunner::discover().await.unwrap();
    RepoLocation::discover(&runner, repo.path()).await.unwrap()
}

fn untracked(repo: &TestRepo) -> Vec<String> {
    repo.git(["ls-files", "--others", "--exclude-standard"])
        .lines()
        .map(str::to_owned)
        .collect()
}

#[tokio::test]
async fn ignores_the_file_at_the_root_and_not_its_namesake_below() {
    let repo = TestRepo::new()
        .write("a.txt", "1\n")
        .commit("start")
        .write("out.log", "x\n")
        .write("logs/out.log", "y\n");
    let loc = open(&repo).await;

    loc.ignore(&exact("out.log")).unwrap();
    assert_eq!(untracked(&repo), [".gitignore", "logs/out.log"]);
}

#[tokio::test]
async fn a_pattern_keeps_every_file_it_matches_out() {
    let repo = TestRepo::new()
        .write("a.txt", "1\n")
        .commit("start")
        .write("out.log", "x\n")
        .write("logs/more.log", "y\n");
    let loc = open(&repo).await;

    loc.ignore("*.log").unwrap();
    loc.ignore("*.log").unwrap();
    assert_eq!(untracked(&repo), [".gitignore"]);
    assert_eq!(repo.git(["show", ":0:a.txt"]).trim(), "1");
    assert_eq!(
        std::fs::read_to_string(repo.path().join(".gitignore")).unwrap(),
        "*.log\n"
    );
}

/// A star in a file name is only possible where the file system allows one.
#[cfg(unix)]
#[tokio::test]
async fn a_star_in_a_name_ignores_that_name_and_is_not_a_wildcard() {
    let repo = TestRepo::new()
        .write("a.txt", "1\n")
        .commit("start")
        .write("a*b.txt", "x\n")
        .write("aXb.txt", "y\n");
    let loc = open(&repo).await;

    loc.ignore(&exact("a*b.txt")).unwrap();
    assert_eq!(untracked(&repo), [".gitignore", "aXb.txt"]);
}
