use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use tokio::sync::mpsc;

use crate::error::CoralError;
use crate::repo::RepoLocation;

/// What changed since the last notification. Never "nothing": an empty set is not emitted.
// A change mask of genuinely independent signals, and the UI branches on them by name. A
// bitfield would serialize as an opaque integer across the IPC boundary.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct RepoChanged {
    /// `refs/**`, `packed-refs`, `HEAD`, or the reflogs.
    pub refs: bool,
    /// The index file.
    pub index: bool,
    /// Anything in the worktree that is not inside the git dir.
    pub worktree: bool,
    /// A multi-step operation started, advanced, or ended.
    pub ops: bool,
    /// The commit-graph file changed, which normally means the background
    /// `git commit-graph write` finished and the walk is now worth redoing.
    pub graph: bool,
}

impl RepoChanged {
    #[must_use]
    pub const fn any(self) -> bool {
        self.refs || self.index || self.worktree || self.ops || self.graph
    }

    fn merge(&mut self, other: Self) {
        self.refs |= other.refs;
        self.index |= other.index;
        self.worktree |= other.worktree;
        self.ops |= other.ops;
        self.graph |= other.graph;
    }
}

/// Trailing debounce. Git writes many files per operation, and a build touches thousands.
const DEBOUNCE: Duration = Duration::from_millis(300);
/// Ceiling on how long a continuous storm can delay a notification.
const MAX_HOLD: Duration = Duration::from_millis(1000);

/// Classifies a changed path relative to a repository.
///
/// Deliberately coarse for the worktree: a `make -j16` in the kernel emits tens of thousands
/// of events a second, and the cost of each must be one boolean, never a path allocation.
#[must_use]
pub fn classify(loc: &RepoLocation, path: &Path) -> Option<RepoChanged> {
    let mut c = RepoChanged::default();

    if let Ok(rel) = path
        .strip_prefix(&loc.common_dir)
        .or_else(|_| path.strip_prefix(&loc.git_dir))
    {
        let rel = rel.to_string_lossy();
        // Objects churn constantly and tell us nothing a ref update will not. The
        // commit-graph is the exception: it appearing means the background write finished.
        if rel.starts_with("objects/") {
            if rel.starts_with("objects/info/commit-graph") {
                c.graph = true;
                return Some(c);
            }
            return None;
        }
        // Our own writes go through a lock file first; reacting to those is noise.
        if rel.ends_with(".lock") {
            return None;
        }
        if rel == "index" {
            c.index = true;
        }
        if rel == "HEAD"
            || rel.starts_with("refs/")
            || rel == "packed-refs"
            || rel.starts_with("logs/")
        {
            c.refs = true;
        }
        if is_op_path(&rel) {
            c.ops = true;
            // An operation starting or finishing moves refs and the index too.
            c.refs = true;
            c.index = true;
        }
        return c.any().then_some(c);
    }

    // Inside the worktree but not the git dir.
    if path.starts_with(loc.display_path()) {
        c.worktree = true;
        return Some(c);
    }
    None
}

/// The files git leaves behind while a multi-step operation is in progress.
fn is_op_path(rel: &str) -> bool {
    const NAMES: [&str; 6] = [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "BISECT_LOG",
        "MERGE_MSG",
        "REBASE_HEAD",
    ];
    NAMES.contains(&rel)
        || rel.starts_with("rebase-merge/")
        || rel.starts_with("rebase-apply/")
        || rel.starts_with("sequencer/")
}

/// Watches one repository and emits coalesced [`RepoChanged`] notifications.
///
/// The receiver is private, and reaching it through [`recv`](Self::recv) is the whole reason.
/// Rust captures closures and async blocks field by field, so `spawn(async move { ...
/// watcher.changes.recv().await ... })` moved only the receiver and left the watcher itself
/// behind to be dropped at the end of the enclosing function. Its file handles went with it,
/// the debouncer's channel disconnected, and the receiver returned `None` a millisecond later
/// — a watch that reported success and then silently never fired. A method call borrows the
/// whole struct, so the same code now keeps it alive.
pub struct RepoWatcher {
    _watcher: notify::RecommendedWatcher,
    changes: mpsc::Receiver<RepoChanged>,
    degraded: Arc<Mutex<Option<String>>>,
}

impl RepoWatcher {
    /// Starts watching `loc`.
    ///
    /// The git dir is always watched. The worktree is best-effort: on Linux, inotify has a
    /// per-user watch limit that a large tree can exhaust, and silently showing stale state is
    /// how a git client earns a reputation for being wrong. [`RepoWatcher::degraded`] reports
    /// that case so the UI can fall back to refreshing on focus.
    ///
    /// # Errors
    /// [`CoralError::Io`] if even the git dir cannot be watched.
    pub fn start(loc: &RepoLocation) -> Result<Self, CoralError> {
        let (raw_tx, raw_rx) = std::sync::mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = raw_tx.send(res);
        })
        .map_err(|e| to_io(&e))?;

        // Refs live in the common dir, which differs from the git dir in a linked worktree.
        watcher
            .watch(&loc.git_dir, RecursiveMode::Recursive)
            .map_err(|e| to_io(&e))?;
        if loc.common_dir != loc.git_dir {
            let _ = watcher.watch(&loc.common_dir, RecursiveMode::Recursive);
        }

        let degraded = Arc::new(Mutex::new(None));
        if let Some(workdir) = &loc.workdir
            && let Err(e) = watcher.watch(workdir, RecursiveMode::Recursive)
        {
            *degraded
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(format!("{e}"));
        }

        let (tx, changes) = mpsc::channel(16);
        spawn_debouncer(loc.clone(), raw_rx, tx);
        Ok(Self {
            _watcher: watcher,
            changes,
            degraded,
        })
    }

    /// The next batch of changes, or `None` once nothing is watching any more.
    pub async fn recv(&mut self) -> Option<RepoChanged> {
        self.changes.recv().await
    }

    /// Set when the worktree could not be watched, with the reason. The UI should refresh on
    /// window focus instead, and say so once.
    #[must_use]
    pub fn degraded(&self) -> Option<String> {
        self.degraded
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

/// Collapses a burst of raw events into at most one notification per [`DEBOUNCE`].
fn spawn_debouncer(
    loc: RepoLocation,
    raw: std::sync::mpsc::Receiver<notify::Result<notify::Event>>,
    tx: mpsc::Sender<RepoChanged>,
) {
    std::thread::spawn(move || {
        let mut pending = RepoChanged::default();
        let mut first_seen: Option<std::time::Instant> = None;

        loop {
            let timeout = if pending.any() {
                DEBOUNCE
            } else {
                Duration::from_secs(3600)
            };
            match raw.recv_timeout(timeout) {
                Ok(Ok(event)) => {
                    for path in &event.paths {
                        if let Some(c) = classify(&loc, path) {
                            pending.merge(c);
                        }
                    }
                    if pending.any() {
                        let start = *first_seen.get_or_insert_with(std::time::Instant::now);
                        // A continuous storm must not postpone the notification forever.
                        if start.elapsed() >= MAX_HOLD {
                            emit(&tx, &mut pending, &mut first_seen);
                        }
                    }
                }
                Ok(Err(_)) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    emit(&tx, &mut pending, &mut first_seen);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
    });
}

fn emit(
    tx: &mpsc::Sender<RepoChanged>,
    pending: &mut RepoChanged,
    first_seen: &mut Option<std::time::Instant>,
) {
    if pending.any() {
        let _ = tx.blocking_send(*pending);
        *pending = RepoChanged::default();
    }
    *first_seen = None;
}

fn to_io(e: &notify::Error) -> CoralError {
    CoralError::Io(std::io::Error::other(e.to_string()))
}

/// Paths a caller needs in order to classify events without a live watcher.
#[must_use]
pub fn watched_roots(loc: &RepoLocation) -> Vec<PathBuf> {
    let mut v = vec![loc.git_dir.clone()];
    if loc.common_dir != loc.git_dir {
        v.push(loc.common_dir.clone());
    }
    if let Some(w) = &loc.workdir {
        v.push(w.clone());
    }
    v
}
