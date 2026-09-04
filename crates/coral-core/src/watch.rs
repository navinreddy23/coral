use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use tokio::sync::mpsc;

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
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

/// What a repository looks like, cheaply enough to ask on every file event.
///
/// Two numbers, not two answers: the point is only whether something differs from last time.
/// They are hashes from the standard library's default hasher, which says nothing across
/// releases or processes and does not need to — nothing is ever stored or compared but two
/// values taken minutes apart by the same running program.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fingerprint {
    pub refs: u64,
    pub status: u64,
}

fn hash(bytes: &[u8]) -> u64 {
    use std::hash::{Hash as _, Hasher as _};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

impl RepoLocation {
    /// A hash of every ref and of what HEAD is on.
    ///
    /// HEAD separately, because `for-each-ref` does not list it and checking a commit out
    /// detached moves nothing else.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn refs_fingerprint(&self, runner: &GitRunner) -> Result<u64, CoralError> {
        let refs = runner
            .output(
                GitCommand::read("for-each-ref", self.display_path())
                    .args(["for-each-ref", "--format=%(objectname) %(refname)"]),
            )
            .await?;
        let head = std::fs::read(self.git_dir.join("HEAD")).unwrap_or_default();
        let mut both = refs.stdout;
        both.extend_from_slice(&head);
        Ok(hash(&both))
    }

    /// A hash of the working tree and index, as the window would show them.
    ///
    /// Read class on purpose, so git does not write the refreshed index back. Writing it is
    /// what makes this loop: the write is a change to the git directory, the watcher reports
    /// it, and asking again is what caused it.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn status_fingerprint(&self, runner: &GitRunner) -> Result<u64, CoralError> {
        let out = runner
            .output(GitCommand::read("status", self.display_path()).args([
                "status",
                "--porcelain=v2",
                "--untracked-files=all",
            ]))
            .await?;
        Ok(hash(&out.stdout))
    }

    /// Drops from `change` everything that turns out not to have changed, updating `seen`.
    ///
    /// A file event says a file was written, which is not the same as the repository being
    /// different: a build writing under an ignored directory, or git refreshing its own index,
    /// both look like change and are not. Returns `None` when nothing is left worth telling
    /// anyone about.
    ///
    /// # Errors
    /// Propagates git failures. A failure here is reported rather than swallowed, since
    /// treating it as "nothing changed" would leave the window stale for good.
    pub async fn narrow(
        &self,
        runner: &GitRunner,
        seen: &mut Fingerprint,
        mut change: RepoChanged,
    ) -> Result<Option<RepoChanged>, CoralError> {
        if change.refs {
            let now = self.refs_fingerprint(runner).await?;
            change.refs = now != seen.refs;
            seen.refs = now;
        }
        if change.index || change.worktree {
            let now = self.status_fingerprint(runner).await?;
            let same = now == seen.status;
            change.index &= !same;
            change.worktree &= !same;
            seen.status = now;
        }
        Ok(change.any().then_some(change))
    }

    /// Both hashes as they stand, for seeding [`RepoLocation::narrow`].
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn fingerprint(&self, runner: &GitRunner) -> Result<Fingerprint, CoralError> {
        Ok(Fingerprint {
            refs: self.refs_fingerprint(runner).await?,
            status: self.status_fingerprint(runner).await?,
        })
    }
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
