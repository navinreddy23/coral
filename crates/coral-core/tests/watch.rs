use std::time::Duration;

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;
use coral_core::watch::{Fingerprint, RepoChanged, RepoWatcher, classify};

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

async fn runner_at(repo: &TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

#[tokio::test]
async fn a_file_event_that_changed_nothing_is_dropped() {
    // The case that made the window flicker. A build writes under an ignored directory and git
    // refreshes its own index; both look exactly like a file changing, and neither is anything
    // the user should see the screen redraw for.
    let repo = TestRepo::new()
        .write("a.txt", "1\n")
        .write(".gitignore", "build/\n")
        .commit("base");
    let (runner, loc) = runner_at(&repo).await;
    let mut seen = loc.fingerprint(&runner).await.unwrap();

    std::fs::create_dir_all(repo.path().join("build")).unwrap();
    std::fs::write(repo.path().join("build/output.bin"), "artefact\n").unwrap();

    let everything = RepoChanged {
        refs: true,
        index: true,
        worktree: true,
        ops: false,
        graph: false,
    };
    let narrowed = loc.narrow(&runner, &mut seen, everything).await.unwrap();
    assert_eq!(narrowed, None, "an ignored file is not a change");
}

#[tokio::test]
async fn a_real_edit_survives_the_narrowing() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let (runner, loc) = runner_at(&repo).await;
    let mut seen = loc.fingerprint(&runner).await.unwrap();

    std::fs::write(repo.path().join("a.txt"), "2\n").unwrap();

    let claimed = RepoChanged {
        worktree: true,
        ..RepoChanged::default()
    };
    let narrowed = loc.narrow(&runner, &mut seen, claimed).await.unwrap();
    assert_eq!(narrowed.map(|c| c.worktree), Some(true));

    // And the second time, with nothing further written, it is no longer news.
    let again = loc.narrow(&runner, &mut seen, claimed).await.unwrap();
    assert_eq!(again, None);
}

#[tokio::test]
async fn a_commit_is_a_ref_change_and_a_status_change() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let (runner, loc) = runner_at(&repo).await;
    let mut seen = loc.fingerprint(&runner).await.unwrap();

    let repo = repo.write("b.txt", "new\n").commit("second");
    let _ = &repo;

    let claimed = RepoChanged {
        refs: true,
        index: true,
        worktree: true,
        ..RepoChanged::default()
    };
    let narrowed = loc
        .narrow(&runner, &mut seen, claimed)
        .await
        .unwrap()
        .expect("a commit changes something");
    assert!(narrowed.refs, "HEAD moved");
}

#[tokio::test]
async fn checking_out_a_commit_counts_even_though_no_ref_moved() {
    // Detached HEAD moves nothing `for-each-ref` lists, so a fingerprint taken from refs alone
    // would report the checkout as nothing at all.
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let repo = repo.write("a.txt", "2\n").commit("second");
    let (runner, loc) = runner_at(&repo).await;
    let mut seen = loc.fingerprint(&runner).await.unwrap();

    repo.git(["checkout", "--quiet", "HEAD~1"]);

    let claimed = RepoChanged {
        refs: true,
        ..RepoChanged::default()
    };
    let narrowed = loc.narrow(&runner, &mut seen, claimed).await.unwrap();
    assert_eq!(narrowed.map(|c| c.refs), Some(true));
}

#[tokio::test]
async fn narrowing_never_invents_a_change_it_was_not_told_about() {
    // The mask says which parts of the repository the file events touched. Narrowing may only
    // take bits away: reporting a ref change because the fingerprint happened to be recomputed
    // would send the window off to rewalk a graph nothing asked about.
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let (runner, loc) = runner_at(&repo).await;
    let mut seen = Fingerprint::default();

    let claimed = RepoChanged {
        ops: true,
        ..RepoChanged::default()
    };
    let narrowed = loc.narrow(&runner, &mut seen, claimed).await.unwrap();
    assert_eq!(narrowed, Some(claimed), "ops passes through untouched");
    assert_eq!(seen, Fingerprint::default(), "and nothing was measured");
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

    // Canonical, because classify strips a prefix lexically and macOS hands out temporary
    // directories under /var, which is a symlink to /private/var. The location reports the
    // resolved path, so an unresolved one here belongs to no repository at all. Not on
    // Windows, where canonicalize answers with a \\?\ extended path git never produces.
    #[cfg(unix)]
    let root = std::fs::canonicalize(repo.path()).unwrap();
    #[cfg(not(unix))]
    let root = repo.path().to_path_buf();
    let c = classify(&loc, &root.join("deep/nested/file.c")).expect("in the worktree");
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

/// Walking the graph reads the git directory, and reading it must not ask for another walk.
///
/// This is the loop that made a large repository never settle: every walk opened the
/// commit-graph, every open raised an event, every event was a change, and the change asked
/// for a walk. The tab sat at six seconds a cycle and a second tab could not be opened at all.
#[tokio::test]
async fn reading_the_git_directory_is_not_a_change() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    repo.git(["commit-graph", "write", "--reachable"]);
    let loc = located(&repo).await;
    let mut w = RepoWatcher::start(&loc).unwrap();

    // Whatever the commit made is now behind us.
    let _ = tokio::time::timeout(Duration::from_secs(1), w.recv()).await;

    for _ in 0..5 {
        for name in ["HEAD", "packed-refs", "objects/info/commit-graph"] {
            let _ = std::fs::read(loc.git_dir.join(name));
        }
    }

    let after = tokio::time::timeout(Duration::from_millis(1500), w.recv())
        .await
        .ok()
        .flatten();
    assert_eq!(after, None, "reading files reported a change");
}
