//! What the log says about a network operation that was stopped.
//!
//! Cancelling drops the work where it stands, so anything that has to be said afterwards has
//! to be owned by something the drop does not take. The activity log is the place somebody
//! opens to find out what happened, and an operation that only ever says "started" there is
//! the worst possible answer to that question.

use coral_app_lib as app;
use coral_core::testutil::TestRepo;

/// The one line the log holds for a repository, as the window would read it.
fn lines(path: &str) -> Vec<String> {
    app::activity::entries(Some(path))
        .into_iter()
        .map(|e| e.message)
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_fetch_that_cannot_reach_its_remote_says_so_in_the_log() {
    // Not a cancellation, but the same shape: the entry is opened before the work and has to
    // be closed by something the work's own failure cannot skip.
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    repo.git(["remote", "add", "nowhere", "/does/not/exist/at/all.git"]);
    let path = repo.path().display().to_string();

    let out = app::actions::run_action(&path, app::actions::Action::Fetch { remote: None }).await;
    assert!(
        out.is_err(),
        "a fetch of a remote that is not there should fail"
    );

    let said = lines(&path);
    assert!(
        said.iter().any(|l| l.contains("started")),
        "the attempt is recorded: {said:?}"
    );
    assert!(
        said.iter().any(|l| l.contains("failed")),
        "and so is how it ended: {said:?}"
    );
}

#[test]
fn an_operation_dropped_without_being_ended_says_nothing_at_all() {
    // The property the fix turns on, asserted directly rather than through a cancellation that
    // a test cannot easily stage. `Operation` records nothing on drop deliberately — putting
    // "finished" in the log for something that was killed would be worse than silence — which
    // is exactly why the entry may not be owned by the work that gets dropped.
    let path = "/tmp/coral-test-dropped-entry";
    let before = app::activity::entries(Some(path)).len();

    drop(app::activity::started(path, "Fetch"));

    let after = app::activity::entries(Some(path));
    assert_eq!(
        after.len(),
        before + 1,
        "only the opening line, and nothing closing it: {after:?}"
    );
    assert!(after.last().unwrap().message.contains("started"));
}

#[test]
fn a_cancellation_is_not_written_down_as_a_failure() {
    // The engine's own words: a cancelled fetch has not gone wrong, it has been called off.
    // "failed" in the log sends somebody looking for a fault that never happened.
    let path = "/tmp/coral-test-cancelled-entry";
    app::activity::started(path, "Fetch").cancelled();

    let last = app::activity::entries(Some(path)).pop().expect("an entry");
    assert!(last.message.contains("cancelled"), "{}", last.message);
    assert!(!last.message.contains("failed"), "{}", last.message);
    assert_ne!(
        last.level,
        app::activity::Level::Error,
        "and it is not coloured as an error"
    );
}
