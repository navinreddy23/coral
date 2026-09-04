use std::time::Duration;

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;
use coral_core::watch::{RepoWatcher, classify};

async fn located(repo: &TestRepo) -> RepoLocation {
    let runner = GitRunner::discover().await.unwrap();
    RepoLocation::discover(&runner, repo.path()).await.unwrap()
}

/// Waits for a notification, giving the debounce time to fire.
async fn next_change(w: &mut RepoWatcher) -> Option<coral_core::watch::RepoChanged> {
    tokio::time::timeout(Duration::from_secs(5), w.recv())
        .await
        .ok()
        .flatten()
}

/// Starts the reading task and returns, which is what the window's command does.
///
/// Returning matters. The bug this covers left the watcher itself behind in the calling
/// function, so it survived exactly as long as that function did; a test that spawned and then
/// waited in the same scope kept it alive by accident and saw nothing wrong.
fn read_in_a_task(
    mut watcher: RepoWatcher,
) -> tokio::sync::mpsc::Receiver<coral_core::watch::RepoChanged> {
    let (tx, rx) = tokio::sync::mpsc::channel(4);
    tokio::spawn(async move {
        while let Some(change) = watcher.recv().await {
            if tx.send(change).await.is_err() {
                return;
            }
        }
    });
    rx
}

#[tokio::test]
async fn keeps_watching_after_the_call_that_started_it_returns() {
    // Rust captures an async block field by field, so reading the receiver directly moved only
    // the receiver: the watcher's own file handles went out of scope with the command that
    // started it, the debouncer's channel disconnected a millisecond later, and the watch
    // reported success and then never fired. It reads as "nothing updates until I switch tabs".
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let loc = located(&repo).await;
    let mut rx = read_in_a_task(RepoWatcher::start(&loc).unwrap());

    // Written after the task is running, so nothing here can be answered from a queued event.
    tokio::time::sleep(Duration::from_millis(200)).await;
    std::fs::write(repo.path().join("a.txt"), "2\n").unwrap();

    let change = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("the watcher stopped when the call that started it returned")
        .expect("the watcher's channel closed");
    assert!(change.worktree, "{change:?}");
}

#[tokio::test]
async fn classifies_git_dir_paths_by_what_they_affect() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let loc = located(&repo).await;
    let git = |rel: &str| classify(&loc, &loc.git_dir.join(rel));

    assert_eq!(git("index").map(|c| c.index), Some(true));
    assert_eq!(git("HEAD").map(|c| c.refs), Some(true));
    assert_eq!(git("refs/heads/main").map(|c| c.refs), Some(true));
    assert_eq!(git("packed-refs").map(|c| c.refs), Some(true));
    assert_eq!(git("logs/HEAD").map(|c| c.refs), Some(true));

    // An in-progress operation moves refs and the index as well.
    let merge = git("MERGE_HEAD").expect("MERGE_HEAD is interesting");
    assert!(merge.ops && merge.refs && merge.index);
    assert!(git("rebase-merge/done").is_some_and(|c| c.ops));
    assert!(git("sequencer/todo").is_some_and(|c| c.ops));
}

/// Object churn and our own lock files are pure noise; reacting to them would mean refreshing
/// constantly during a fetch or our own writes.
#[tokio::test]
async fn ignores_object_churn_and_lock_files() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let loc = located(&repo).await;
    let git = |rel: &str| classify(&loc, &loc.git_dir.join(rel));

    assert_eq!(git("objects/ab/cdef123"), None);
    assert_eq!(git("objects/pack/pack-abc.pack"), None);
    assert_eq!(git("index.lock"), None);
    assert_eq!(git("refs/heads/main.lock"), None);

    // The commit-graph is the one thing under objects/ worth hearing about.
    assert!(git("objects/info/commit-graph").is_some());
}

#[tokio::test]
async fn classifies_worktree_edits_without_naming_paths() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let loc = located(&repo).await;

    let c = classify(&loc, &repo.path().join("deep/nested/file.c")).expect("in the worktree");
    assert!(c.worktree);
    assert!(!c.index && !c.refs && !c.ops);

    assert_eq!(
        classify(&loc, std::path::Path::new("/elsewhere/file")),
        None
    );
}

#[tokio::test]
async fn notices_a_worktree_edit() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let loc = located(&repo).await;
    let mut w = RepoWatcher::start(&loc).unwrap();

    std::fs::write(repo.path().join("a.txt"), "changed\n").unwrap();
    let change = next_change(&mut w)
        .await
        .expect("a worktree edit should notify");
    assert!(change.worktree);
}

#[tokio::test]
async fn notices_a_commit_as_a_ref_and_index_change() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let loc = located(&repo).await;
    let mut w = RepoWatcher::start(&loc).unwrap();

    let repo = repo.write("a.txt", "2\n").commit("second");
    drop(repo);

    let change = next_change(&mut w).await.expect("a commit should notify");
    assert!(
        change.refs || change.index,
        "a commit moves HEAD and the index"
    );
}

/// A build touching thousands of files must not produce thousands of notifications.
#[tokio::test]
async fn a_burst_of_edits_coalesces() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let loc = located(&repo).await;
    let mut w = RepoWatcher::start(&loc).unwrap();

    for i in 0..500 {
        std::fs::write(repo.path().join(format!("f{i}.txt")), "x\n").unwrap();
    }

    let first = next_change(&mut w)
        .await
        .expect("the burst should notify once");
    assert!(first.worktree);

    // Whatever else arrives must be a small number of coalesced batches, not one per file.
    let mut extra = 0;
    while tokio::time::timeout(Duration::from_millis(600), w.recv())
        .await
        .ok()
        .flatten()
        .is_some()
    {
        extra += 1;
        assert!(extra < 20, "500 edits produced more than 20 notifications");
    }
}
