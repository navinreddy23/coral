//! A network operation the user can watch and stop.
//!
//! Fetch, push and clone are the only things Coral does that are bounded by somebody else's
//! server rather than by this machine. They are also the only ones with no timeout, on
//! purpose: a clone of a large repository is legitimately minutes long and a clock cannot tell
//! that from a hang. What makes that safe is being able to stop it, and until this existed
//! nothing could — the process runner's comment said a network command was bounded by the
//! caller cancelling, and no caller cancelled.
//!
//! Stopping is dropping the work. The runner spawns its children with `kill_on_drop`, so
//! aborting the task kills git wherever it has got to, which is what makes a wedged connection
//! recoverable without killing the window.

// Tauri requires `State` by value in a command signature; it is a handle, not the data.
#![allow(clippy::needless_pass_by_value)]

use std::collections::HashMap;
use std::sync::Mutex;

use tauri::{Emitter as _, Manager as _};

/// The event the window listens on.
pub const EVENT: &str = "coral://transfer";

/// Where a transfer has got to.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum State {
    /// Started, or moved on. Carries progress once git reports any.
    Running,
    Finished,
    Failed,
    Cancelled,
}

/// One report about one transfer.
///
/// The first is sent before git has said anything, so the window can show that something is
/// happening and offer the way out immediately. That matters most in the case this exists
/// for: a host that never answers produces no progress at all, and a cancel that only appears
/// once progress arrives would never appear.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// What the window cancels by, and what it matches its own view against.
    pub key: String,
    /// "Clone", "Fetch", "Push".
    pub label: String,
    pub state: State,
    pub phase: String,
    pub current: u64,
    pub total: u64,
    pub percent: u8,
    /// The work is on the server rather than on this machine.
    pub remote: bool,
    /// Why it failed, when it did.
    pub detail: String,
}

impl Report {
    fn new(key: &str, label: &str, state: State) -> Self {
        Self {
            key: key.to_owned(),
            label: label.to_owned(),
            state,
            phase: String::new(),
            current: 0,
            total: 0,
            percent: 0,
            remote: false,
            detail: String::new(),
        }
    }
}

/// Every transfer that is running, and the switch that stops it.
#[derive(Default)]
pub struct Transfers {
    inner: Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>,
}

type Held<'a> = std::sync::MutexGuard<'a, HashMap<String, tokio::sync::oneshot::Sender<()>>>;

impl Transfers {
    fn held(&self) -> Held<'_> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Stops the transfer under `key`, answering whether there was one.
    pub fn cancel(&self, key: &str) -> bool {
        // The receiver also fires when this is dropped, so a send to a transfer that has just
        // finished is not an error; there is simply nothing there.
        self.held().remove(key).is_some_and(|s| s.send(()).is_ok())
    }
}

/// Runs a network operation, reporting it and leaving it stoppable while it runs.
///
/// Cancelling drops the work rather than asking it to stop. Nothing in git's protocol offers a
/// polite way out, and there is nothing to poll while a connection hangs, so the only honest
/// mechanism is to let the future go and let the runner kill the child on the way past. A
/// clone takes its half-made directory with it.
///
/// # Errors
/// Whatever the operation returns, and [`coral_core::CoralError::Cancelled`] when it was
/// stopped.
pub async fn watched<T, E, F, Fut>(
    app: &tauri::AppHandle,
    key: &str,
    label: &str,
    work: F,
) -> Result<T, E>
where
    E: From<coral_core::CoralError> + std::fmt::Display,
    F: FnOnce(Sender) -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let sender = Sender {
        app: app.clone(),
        key: key.to_owned(),
        label: label.to_owned(),
    };
    // Before git has said anything. A host that never answers produces no progress at all, and
    // a way out that only appears with the first record would never appear.
    sender.send(Report::new(key, label, State::Running));

    let transfers = app.state::<Transfers>();
    let (stop, stopped) = tokio::sync::oneshot::channel();
    transfers.held().insert(key.to_owned(), stop);

    let outcome = tokio::select! {
        done = work(sender.clone()) => Some(done),
        _ = stopped => None,
    };
    transfers.held().remove(key);

    match outcome {
        Some(Ok(value)) => {
            sender.send(Report::new(key, label, State::Finished));
            Ok(value)
        }
        Some(Err(e)) => {
            let mut report = Report::new(key, label, State::Failed);
            report.detail = e.to_string();
            sender.send(report);
            Err(e)
        }
        None => {
            sender.send(Report::new(key, label, State::Cancelled));
            Err(coral_core::CoralError::Cancelled { label: "transfer" }.into())
        }
    }
}

/// Reports progress for one transfer. Cloned into the operation, which calls it from git's
/// own output.
#[derive(Clone)]
pub struct Sender {
    app: tauri::AppHandle,
    key: String,
    label: String,
}

impl Sender {
    /// Passes on one of git's progress records.
    pub fn progress(&self, p: &coral_core::remote::Progress) {
        self.send(Report {
            key: self.key.clone(),
            label: self.label.clone(),
            state: State::Running,
            phase: p.phase.clone(),
            current: p.current,
            total: p.total,
            percent: p.percent,
            remote: p.remote,
            detail: String::new(),
        });
    }

    fn send(&self, report: Report) {
        // A window that has gone is not a reason to fail an operation that is otherwise fine.
        if let Err(e) = self.app.emit(EVENT, report) {
            tracing::debug!(error = %e, "could not report a transfer");
        }
    }
}

/// Stops a running transfer.
///
/// Answers whether there was one to stop rather than failing, since the honest race is that it
/// finished between the window drawing the button and the user pressing it.
#[tauri::command]
#[must_use]
pub fn cancel_transfer(transfers: tauri::State<'_, Transfers>, key: String) -> bool {
    transfers.cancel(&key)
}

/// Every transfer running right now, so a window that reloaded can find its way back to one.
#[tauri::command]
#[must_use]
pub fn running_transfers(transfers: tauri::State<'_, Transfers>) -> Vec<String> {
    transfers.held().keys().cloned().collect()
}
