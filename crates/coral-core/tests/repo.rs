use coral_core::process::GitRunner;
use coral_core::repo::{Head, OpState, RepoLocation, present};
use coral_core::testutil::TestRepo;

async fn runner() -> GitRunner {
    GitRunner::discover()
        .await
        .expect("git on PATH and new enough")
}

#[tokio::test]
async fn discovers_a_normal_repository() {
    let fixture = TestRepo::new().write("a.txt", "hello\n").commit("first");
    let r = runner().await;
    let loc = RepoLocation::discover(&r, fixture.path()).await.unwrap();

    assert!(!loc.is_bare);
    assert!(loc.workdir.is_some());
    assert_eq!(loc.git_dir, loc.common_dir);
    assert_eq!(loc.op_state(), OpState::Clean);
    assert_eq!(
        loc.head(&r).await.unwrap(),
        Head::Branch {
            name: "main".to_owned()
        }
    );
}

#[tokio::test]
async fn reports_an_unborn_head_before_the_first_commit() {
    let fixture = TestRepo::new();
    let r = runner().await;
    let loc = RepoLocation::discover(&r, fixture.path()).await.unwrap();

    assert_eq!(
        loc.head(&r).await.unwrap(),
        Head::Unborn {
            name: "main".to_owned()
        }
    );
}

#[tokio::test]
async fn reports_a_detached_head() {
    let fixture = TestRepo::new().write("a.txt", "1\n").commit("first");
    let oid = fixture.git(["rev-parse", "HEAD"]);
    fixture.git(["checkout", "--quiet", "--detach", &oid]);

    let r = runner().await;
    let loc = RepoLocation::discover(&r, fixture.path()).await.unwrap();
    assert_eq!(loc.head(&r).await.unwrap(), Head::Detached { oid });
}

#[tokio::test]
async fn discovers_from_a_subdirectory() {
    let fixture = TestRepo::new()
        .write("sub/deep/a.txt", "x\n")
        .commit("first");
    let r = runner().await;
    let loc = RepoLocation::discover(&r, &fixture.path().join("sub/deep"))
        .await
        .unwrap();

    let found = std::fs::canonicalize(loc.workdir.unwrap()).unwrap();
    assert_eq!(found, std::fs::canonicalize(fixture.path()).unwrap());
}

#[tokio::test]
async fn discovers_a_linked_worktree_with_a_distinct_git_dir() {
    let fixture = TestRepo::new().write("a.txt", "1\n").commit("first");
    let wt = fixture.path().join("wt-linked");
    fixture.git([
        "worktree",
        "add",
        "--quiet",
        wt.to_str().unwrap(),
        "-b",
        "side",
    ]);

    let r = runner().await;
    let loc = RepoLocation::discover(&r, &wt).await.unwrap();
    assert_ne!(
        loc.git_dir, loc.common_dir,
        "a linked worktree has its own git dir"
    );
    assert_eq!(
        loc.head(&r).await.unwrap(),
        Head::Branch {
            name: "side".to_owned()
        }
    );
}

#[tokio::test]
async fn detects_a_bare_repository() {
    let fixture = TestRepo::new().write("a.txt", "1\n").commit("first");
    let bare = fixture.path().join("bare.git");
    fixture.git(["clone", "--quiet", "--bare", ".", bare.to_str().unwrap()]);

    let r = runner().await;
    let loc = RepoLocation::discover(&r, &bare).await.unwrap();
    assert!(loc.is_bare);
    assert!(loc.workdir.is_none());
}

#[tokio::test]
async fn rejects_a_directory_that_is_not_a_repository() {
    let dir = tempfile::tempdir().unwrap();
    let r = runner().await;
    let err = RepoLocation::discover(&r, dir.path()).await.unwrap_err();
    assert_eq!(err.code(), "not_a_repository");
}

#[tokio::test]
async fn detects_an_in_progress_merge() {
    let fixture = TestRepo::new().write("a.txt", "base\n").commit("base");
    fixture.git(["checkout", "--quiet", "-b", "side"]);
    let fixture = fixture.write("a.txt", "side\n").commit("side");
    fixture.git(["checkout", "--quiet", "main"]);
    let fixture = fixture.write("a.txt", "main\n").commit("main");

    // Expected to conflict, so bypass the fixture helper's success assertion.
    std::process::Command::new("git")
        .current_dir(fixture.path())
        .args(["merge", "side"])
        .output()
        .unwrap();

    let r = runner().await;
    let loc = RepoLocation::discover(&r, fixture.path()).await.unwrap();
    assert_eq!(loc.op_state(), OpState::Merge);
}

#[test]
fn a_repository_that_has_gone_is_not_present() {
    let fixture = TestRepo::new().write("a.txt", "hello\n").commit("first");
    assert!(present(fixture.path()));

    // A bare repository has no .git, and the start page lists those too.
    let bare = fixture.path().join("bare.git");
    fixture.git(["clone", "--quiet", "--bare", ".", bare.to_str().unwrap()]);
    assert!(present(&bare));

    std::fs::remove_dir_all(&bare).unwrap();
    assert!(!present(&bare));
    assert!(!present(&fixture.path().join("never-existed")));
}

#[tokio::test]
async fn the_git_directory_stands_for_the_repository_holding_it() {
    // What a file chooser hands back when somebody picks the folder they think of as the
    // repository. git refuses to name a work tree from inside one, so this was a failure
    // about a directory nobody meant to open.
    let fixture = TestRepo::new().write("a.txt", "hello\n").commit("first");
    let r = runner().await;

    let loc = RepoLocation::discover(&r, &fixture.path().join(".git"))
        .await
        .unwrap();
    assert!(!loc.is_bare);
    assert_eq!(
        loc.workdir.map(|w| std::fs::canonicalize(w).unwrap()),
        Some(std::fs::canonicalize(fixture.path()).unwrap()),
    );
}

#[tokio::test]
async fn a_bare_repository_named_for_itself_is_not_mistaken_for_one() {
    // `project.git` is a repository, not the git dir of the directory above it.
    let fixture = TestRepo::new().write("a.txt", "hello\n").commit("first");
    let bare = fixture.path().join("bare.git");
    fixture.git(["clone", "--quiet", "--bare", ".", bare.to_str().unwrap()]);

    let r = runner().await;
    let loc = RepoLocation::discover(&r, &bare).await.unwrap();
    assert!(loc.is_bare);
    assert_eq!(loc.workdir, None);
}
