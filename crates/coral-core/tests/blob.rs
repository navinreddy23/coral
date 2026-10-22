use coral_core::blob::BlobSide;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

async fn open(repo: &TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

/// Bytes that are not text and not valid UTF-8, so nothing on the way may decode them.
const BEFORE: &[u8] = &[0x89, b'P', b'N', b'G', 0x00, 0xff, 0xfe, 1];
const AFTER: &[u8] = &[0x89, b'P', b'N', b'G', 0x00, 0xff, 0xfe, 2, 3];

fn write(repo: &TestRepo, path: &str, bytes: &[u8]) {
    std::fs::write(repo.path().join(path), bytes).unwrap();
}

fn head(repo: &TestRepo) -> String {
    repo.git(["rev-parse", "HEAD"]).trim().to_owned()
}

#[tokio::test]
async fn reads_both_sides_of_a_change_byte_for_byte() {
    let repo = TestRepo::new().write("readme.txt", "x\n").commit("start");
    write(&repo, "logo.png", BEFORE);
    repo.git(["add", "logo.png"]);
    repo.git(["commit", "--quiet", "-m", "add the logo"]);
    let added = head(&repo);
    write(&repo, "logo.png", AFTER);
    repo.git(["commit", "--quiet", "-am", "change the logo"]);
    let changed = head(&repo);
    let (runner, loc) = open(&repo).await;

    let blob = |side: BlobSide| {
        let (runner, loc) = (runner.clone(), loc.clone());
        async move { loc.blob(&runner, &side, "logo.png").await.unwrap() }
    };
    assert_eq!(
        blob(BlobSide::Parent {
            rev: changed.clone()
        })
        .await
        .as_deref(),
        Some(BEFORE)
    );
    assert_eq!(
        blob(BlobSide::Commit { rev: changed }).await.as_deref(),
        Some(AFTER)
    );
    // Before an addition there is nothing.
    assert_eq!(blob(BlobSide::Parent { rev: added }).await, None);
}

#[tokio::test]
async fn a_root_commit_has_no_before() {
    let repo = TestRepo::new();
    write(&repo, "logo.png", BEFORE);
    repo.git(["add", "logo.png"]);
    repo.git(["commit", "--quiet", "-m", "root"]);
    let (runner, loc) = open(&repo).await;
    let side = BlobSide::Parent { rev: head(&repo) };
    assert_eq!(loc.blob(&runner, &side, "logo.png").await.unwrap(), None);
}

#[tokio::test]
async fn reads_the_index_and_the_working_tree() {
    let repo = TestRepo::new();
    write(&repo, "logo.png", BEFORE);
    repo.git(["add", "logo.png"]);
    repo.git(["commit", "--quiet", "-m", "root"]);
    write(&repo, "logo.png", AFTER);
    let (runner, loc) = open(&repo).await;
    assert_eq!(
        loc.blob(&runner, &BlobSide::Index, "logo.png")
            .await
            .unwrap()
            .as_deref(),
        Some(BEFORE)
    );
    assert_eq!(
        loc.blob(&runner, &BlobSide::Worktree, "logo.png")
            .await
            .unwrap()
            .as_deref(),
        Some(AFTER)
    );
    // A file deleted from the working tree has no after.
    std::fs::remove_file(repo.path().join("logo.png")).unwrap();
    assert_eq!(
        loc.blob(&runner, &BlobSide::Worktree, "logo.png")
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn will_not_read_outside_the_working_tree() {
    let repo = TestRepo::new().write("a.txt", "a\n").commit("start");
    let (runner, loc) = open(&repo).await;
    let err = loc
        .blob(&runner, &BlobSide::Worktree, "../outside.png")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "refused");
}
