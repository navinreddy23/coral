//! What the application has been doing, kept in memory for the window to show.
//!
//! Two channels, because the two questions are different. The repository channel answers "what
//! did Coral do to my repository, and when" — one line when an operation starts and one when it
//! finishes, with how long it took. The application channel answers "why is this build
//! misbehaving" and is fed by `tracing`, so every warning the code already emits appears there
//! without a second call site.
//!
//! In memory and bounded. A log that grows without limit is a leak, and one written to disk is
//! a file nobody knew was being written; what is wanted here is the last few hundred things
//! that happened, available while the window is open.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// How many entries each channel keeps. Enough to cover a working session; small enough that
/// the whole log is a cheap thing to hand the window on every open.
const CAPACITY: usize = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Info,
    Warn,
    Error,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// Milliseconds since the Unix epoch, so the window formats the time in the user's locale
    /// rather than the log deciding for it.
    pub at: f64,
    pub level: Level,
    /// The repository this belongs to, or `None` for an application entry.
    pub repo: Option<String>,
    pub message: String,
    /// How long the operation took, on the line that ends one.
    pub millis: Option<u64>,
}

/// The log itself. One buffer, not two: an entry knows which channel it belongs to by whether
/// it names a repository, and one lock is one lock.
struct Log(Mutex<VecDeque<Entry>>);

fn log() -> &'static Log {
    static LOG: OnceLock<Log> = OnceLock::new();
    LOG.get_or_init(|| Log(Mutex::new(VecDeque::with_capacity(CAPACITY))))
}

fn now_millis() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

fn push(entry: Entry) {
    // A poisoned lock must not take the window with it: losing a log line is not worth a panic
    // in the middle of whatever was being logged.
    let Ok(mut entries) = log().0.lock() else {
        return;
    };
    append(&mut entries, entry);
}

/// Adds one entry, dropping the oldest once the buffer is full.
///
/// Takes the buffer rather than reaching for the global one so that the bound can be tested
/// without a test filling the log every other test is reading.
fn append(entries: &mut VecDeque<Entry>, entry: Entry) {
    if entries.len() >= CAPACITY {
        entries.pop_front();
    }
    entries.push_back(entry);
}

/// Records an application entry. Called by the `tracing` layer, and directly where there is no
/// span to hang it on.
pub fn app(level: Level, message: impl Into<String>) {
    push(Entry {
        at: now_millis(),
        level,
        repo: None,
        message: message.into(),
        millis: None,
    });
}

/// Records a repository entry that is not an operation with a duration.
pub fn note(repo: &str, level: Level, message: impl Into<String>) {
    push(Entry {
        at: now_millis(),
        level,
        repo: Some(repo.to_owned()),
        message: message.into(),
        millis: None,
    });
}

/// Records that an operation on a repository has started, and returns the handle that ends it.
///
/// Both ends are logged rather than only the finish, because an operation that never finishes
/// is exactly the one worth seeing in the log.
#[must_use]
pub fn started(repo: &str, what: &str) -> Operation {
    note(repo, Level::Info, format!("{what}: started."));
    Operation {
        repo: repo.to_owned(),
        what: what.to_owned(),
        since: Instant::now(),
    }
}

/// An operation in progress. Ending it is explicit: dropping one without saying how it went
/// would put "finished" in the log for something that failed.
pub struct Operation {
    repo: String,
    what: String,
    since: Instant,
}

impl Operation {
    fn end(self, level: Level, outcome: &str) {
        let millis = u64::try_from(self.since.elapsed().as_millis()).unwrap_or(u64::MAX);
        push(Entry {
            at: now_millis(),
            level,
            repo: Some(self.repo),
            message: format!("{}: {outcome}", self.what),
            millis: Some(millis),
        });
    }

    pub fn finished(self) {
        self.end(Level::Info, "finished.");
    }

    pub fn failed(self, why: &str) {
        self.end(Level::Error, &format!("failed. {why}"));
    }

    /// Called off by the user. Not an error, and reading one in the log sends somebody
    /// looking for a fault that never happened.
    pub fn cancelled(self) {
        self.end(Level::Info, "cancelled.");
    }

    /// Ran, and did not do what it set out to do: a merge that stopped on conflicts, a push
    /// the remote rejected. Not a failure — git did what it was asked and reported back — but
    /// "finished" is not true either, and this log is what somebody reads afterwards to find
    /// out what happened.
    pub fn stopped(self) {
        self.end(Level::Warn, "did not complete.");
    }
}

/// Every entry for one repository, oldest first, or every application entry when `repo` is
/// `None`.
#[must_use]
pub fn entries(repo: Option<&str>) -> Vec<Entry> {
    let Ok(entries) = log().0.lock() else {
        return Vec::new();
    };
    entries
        .iter()
        .filter(|e| e.repo.as_deref() == repo)
        .cloned()
        .collect()
}

/// Forgets everything in one channel. The window offers this because a log is most readable
/// when it starts at the thing being investigated.
pub fn clear(repo: Option<&str>) {
    let Ok(mut entries) = log().0.lock() else {
        return;
    };
    entries.retain(|e| e.repo.as_deref() != repo);
}

/// A `tracing` layer that copies what the code already logs into the application channel.
///
/// A layer rather than a call at every site: the warnings worth reading — a watcher that could
/// not start, a credential helper that was not configured — are already written, and a second
/// set of calls beside them would drift out of step with the first.
pub struct Capture;

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Capture {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let level = match *event.metadata().level() {
            tracing::Level::ERROR => Level::Error,
            tracing::Level::WARN => Level::Warn,
            _ => Level::Info,
        };
        let mut visitor = Message(String::new());
        event.record(&mut visitor);
        if !visitor.0.is_empty() {
            app(level, visitor.0);
        }
    }
}

/// Flattens an event into one line: the message, then any other fields it carried.
struct Message(String);

impl tracing::field::Visit for Message {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        use std::fmt::Write as _;
        if field.name() == "message" {
            // `format_args!` prints without quotes, which is what makes this read as a sentence
            // rather than as a debug dump.
            let _ = write!(self.0, "{value:?}");
        } else {
            let _ = write!(self.0, " {}={value:?}", field.name());
        }
    }
}

/// Everything logged for one repository, or the application's own log when `repo` is absent.
// Tauri deserializes command arguments, so they arrive owned whether or not the body keeps
// them; taking a reference here is not something the caller can do.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
#[must_use]
pub fn activity_log(repo: Option<String>) -> Vec<Entry> {
    entries(repo.as_deref())
}

/// Empties one channel.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn activity_clear(repo: Option<String>) {
    clear(repo.as_deref());
}

#[cfg(test)]
mod tests {
    use super::{CAPACITY, Entry, Level, append, clear, entries, note, started};
    use std::collections::VecDeque;

    fn entry(repo: Option<&str>) -> Entry {
        Entry {
            at: 0.0,
            level: Level::Info,
            repo: repo.map(str::to_owned),
            message: "x".to_owned(),
            millis: None,
        }
    }

    #[test]
    fn drops_the_oldest_once_it_is_full() {
        // A log that grows without limit is a leak, and this one is fed by every git operation
        // and every warning the app emits.
        let mut buffer = VecDeque::new();
        for i in 0..CAPACITY + 10 {
            let mut e = entry(None);
            e.message = i.to_string();
            append(&mut buffer, e);
        }
        assert_eq!(buffer.len(), CAPACITY);
        assert_eq!(buffer.front().map(|e| e.message.as_str()), Some("10"));
        assert_eq!(
            buffer.back().map(|e| e.message.as_str()),
            Some((CAPACITY + 9).to_string()).as_deref()
        );
    }

    #[test]
    fn brackets_an_operation_with_a_start_and_a_finish() {
        let repo = "/tmp/coral-activity-finish";
        started(repo, "Pull").finished();

        let log = entries(Some(repo));
        assert_eq!(log.len(), 2, "{log:?}");
        assert_eq!(log[0].message, "Pull: started.");
        assert_eq!(log[1].message, "Pull: finished.");
        // Only the line that ends the operation carries how long it took.
        assert!(log[0].millis.is_none());
        assert!(log[1].millis.is_some());
    }

    #[test]
    fn says_why_when_an_operation_fails() {
        let repo = "/tmp/coral-activity-fail";
        started(repo, "Push").failed("remote rejected the update");

        let log = entries(Some(repo));
        assert_eq!(log.len(), 2, "{log:?}");
        assert_eq!(log[1].level, Level::Error);
        assert!(log[1].message.contains("remote rejected"), "{:?}", log[1]);
    }

    #[test]
    fn keeps_the_two_channels_and_every_repository_apart() {
        // The window shows one repository at a time, and showing another's operations under its
        // name is the bug this separation exists to prevent.
        let mine = "/tmp/coral-activity-mine";
        let theirs = "/tmp/coral-activity-theirs";
        note(mine, Level::Info, "only mine");
        note(theirs, Level::Info, "only theirs");

        let log = entries(Some(mine));
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].message, "only mine");
        assert!(!entries(None).iter().any(|e| e.message == "only mine"));
    }

    #[test]
    fn clearing_one_channel_leaves_the_others() {
        let cleared = "/tmp/coral-activity-cleared";
        let kept = "/tmp/coral-activity-kept";
        note(cleared, Level::Info, "goes");
        note(kept, Level::Info, "stays");

        clear(Some(cleared));
        assert!(entries(Some(cleared)).is_empty());
        assert_eq!(entries(Some(kept)).len(), 1);
    }
}
