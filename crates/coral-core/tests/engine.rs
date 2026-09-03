use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use coral_core::engine::{Engine, ReadKey};
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;
use coral_core::watch::RepoChanged;

async fn engine(repo: &TestRepo) -> Engine {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    Engine::new(loc, runner)
}

fn worktree_change() -> RepoChanged {
    RepoChanged {
        worktree: true,
        ..RepoChanged::default()
    }
}

/// Fifty simultaneous callers asking the same question must spawn one git child, not fifty.
#[tokio::test(flavor = "multi_thread")]
async fn concurrent_reads_of_the_same_key_run_once() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let engine = Arc::new(engine(&repo).await);
    let runs = Arc::new(AtomicUsize::new(0));

    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..50 {
        let (engine, runs) = (Arc::clone(&engine), Arc::clone(&runs));
        tasks.spawn(async move {
            engine
                .read(ReadKey::Status, || async {
                    runs.fetch_add(1, Ordering::SeqCst);
                    // Long enough that the other callers are certain to arrive mid-flight.
                    tokio::time::sleep(Duration::from_millis(150)).await;
                    Ok(7_u32)
                })
                .await
                .map(|v| *v)
        });
    }

    let mut results = Vec::new();
    while let Some(r) = tasks.join_next().await {
        results.push(r.unwrap().unwrap());
    }

    assert_eq!(results.len(), 50);
    assert!(
        results.iter().all(|v| *v == 7),
        "every caller gets the same answer"
    );
    assert_eq!(
        runs.load(Ordering::SeqCst),
        1,
        "the read ran more than once"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn different_keys_do_not_share_a_result() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let engine = engine(&repo).await;

    let status = engine
        .read(ReadKey::Status, || async { Ok(1_u32) })
        .await
        .unwrap();
    let refs = engine
        .read(ReadKey::Refs, || async { Ok(2_u32) })
        .await
        .unwrap();

    assert_eq!((*status, *refs), (1, 2));
}

/// A read that began before a change was observed is stale. Joining it would show the user
/// state from before the edit they just made.
#[tokio::test(flavor = "multi_thread")]
async fn a_change_mid_flight_forces_a_fresh_read() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let engine = Arc::new(engine(&repo).await);
    let runs = Arc::new(AtomicUsize::new(0));

    let first = {
        let (engine, runs) = (Arc::clone(&engine), Arc::clone(&runs));
        tokio::spawn(async move {
            engine
                .read(ReadKey::Status, || async {
                    runs.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    Ok(1_u32)
                })
                .await
                .map(|v| *v)
        })
    };

    tokio::time::sleep(Duration::from_millis(40)).await;
    engine.observe(worktree_change());

    let second = engine
        .read(ReadKey::Status, || async {
            runs.fetch_add(1, Ordering::SeqCst);
            Ok(2_u32)
        })
        .await
        .unwrap();

    assert_eq!(
        *second, 2,
        "the later caller must not receive the stale answer"
    );
    assert_eq!(first.await.unwrap().unwrap(), 1);
    assert_eq!(runs.load(Ordering::SeqCst), 2, "both reads had to run");
}

#[tokio::test(flavor = "multi_thread")]
async fn observing_a_change_bumps_the_epoch_and_notifies() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let engine = engine(&repo).await;
    let mut events = engine.subscribe();

    assert_eq!(engine.epoch(), 0);
    engine.observe(worktree_change());
    assert_eq!(engine.epoch(), 1);

    let event = tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    match event {
        coral_core::engine::RepoEvent::Changed { epoch, changed } => {
            assert_eq!(epoch, 1);
            assert!(changed.worktree);
        }
        other @ coral_core::engine::RepoEvent::Degraded { .. } => {
            panic!("unexpected event: {other:?}")
        }
    }
}

/// An empty change set is not a change; emitting it would refresh the UI for nothing.
#[tokio::test(flavor = "multi_thread")]
async fn an_empty_change_is_ignored() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let engine = engine(&repo).await;

    engine.observe(RepoChanged::default());
    assert_eq!(engine.epoch(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failing_read_is_reported_and_not_cached() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let engine = engine(&repo).await;

    let failed: Result<Arc<u32>, _> = engine
        .read(ReadKey::Status, || async {
            Err(coral_core::CoralError::NotARepository("/nowhere".into()))
        })
        .await;
    assert_eq!(failed.unwrap_err().code(), "not_a_repository");

    // The failure must not be remembered as the answer.
    let ok = engine
        .read(ReadKey::Status, || async { Ok(5_u32) })
        .await
        .unwrap();
    assert_eq!(*ok, 5);
}

/// The engine drives real reads, not just closures.
#[tokio::test(flavor = "multi_thread")]
async fn drives_a_real_status_read() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let repo = repo.write("a.txt", "changed\n");
    let engine = engine(&repo).await;

    let status = engine
        .read(ReadKey::Status, || async {
            engine.location().status(engine.runner()).await
        })
        .await
        .unwrap();

    assert_eq!(status.branch.as_deref(), Some("main"));
    assert_eq!(status.entries.len(), 1);
}
