use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::{RwLock, Semaphore, broadcast};

use crate::error::CoralError;
use crate::process::GitRunner;
use crate::repo::RepoLocation;
use crate::watch::RepoChanged;

/// Concurrent git children per repository. Enough to overlap a status with a graph walk
/// without letting a burst of reads starve the machine.
const MAX_CHILDREN: usize = 4;

/// Identifies a cacheable read. Two callers asking the same question join one execution.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ReadKey {
    Status,
    Refs,
    Graph,
    Log(String),
    Diff { staged: bool },
    Blame { rev: String, path: String },
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RepoEvent {
    Changed {
        epoch: u64,
        changed: RepoChanged,
    },
    /// The worktree could not be watched; the UI must refresh on focus instead.
    Degraded {
        reason: String,
    },
}

/// Per-repository read scheduling.
///
/// Reads run concurrently under a semaphore and are deduplicated by [`ReadKey`]: fifty
/// simultaneous `status()` calls spawn one git child, not fifty. A result computed before a
/// change was observed is stale, so callers arriving after that change start a fresh run
/// rather than joining the in-flight one — joining it is the classic bug where the UI shows
/// state from before the file you just saved.
pub struct Engine {
    loc: RepoLocation,
    runner: GitRunner,
    children: Arc<Semaphore>,
    inflight: std::sync::Mutex<HashMap<ReadKey, Inflight>>,
    /// Read guard for anything that touches `.git/index`; write guard for mutations. Used as
    /// a barrier rather than to protect data. `tokio`'s `RwLock` is FIFO-fair, which is
    /// load-bearing: a stream of status refreshes during a rebase must not starve the writer.
    index_gate: RwLock<()>,
    epoch: AtomicU64,
    events: broadcast::Sender<RepoEvent>,
}

type AnyResult = Result<Arc<dyn std::any::Any + Send + Sync>, CoralError>;

/// Whether this caller runs the read or waits on someone who is.
enum Claim {
    Join(tokio::sync::watch::Receiver<Option<AnyResult>>),
    Run(tokio::sync::watch::Sender<Option<AnyResult>>),
}

struct Inflight {
    epoch_started: u64,
    result: tokio::sync::watch::Receiver<Option<AnyResult>>,
}

impl Engine {
    #[must_use]
    pub fn new(loc: RepoLocation, runner: GitRunner) -> Self {
        let (events, _) = broadcast::channel(64);
        Self {
            loc,
            runner,
            children: Arc::new(Semaphore::new(MAX_CHILDREN)),
            inflight: std::sync::Mutex::new(HashMap::new()),
            index_gate: RwLock::new(()),
            epoch: AtomicU64::new(0),
            events,
        }
    }

    #[must_use]
    pub const fn location(&self) -> &RepoLocation {
        &self.loc
    }

    #[must_use]
    pub const fn runner(&self) -> &GitRunner {
        &self.runner
    }

    #[must_use]
    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<RepoEvent> {
        self.events.subscribe()
    }

    /// Records that the repository changed, invalidating in-flight reads.
    pub fn observe(&self, changed: RepoChanged) {
        if !changed.any() {
            return;
        }
        let epoch = self.epoch.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = self.events.send(RepoEvent::Changed { epoch, changed });
    }

    /// Runs `f`, or joins an equivalent read already in flight.
    ///
    /// Deduplication covers work that is running, not work that has finished: results are not
    /// memoized, so a caller arriving after completion runs again. Caching answers would mean
    /// serving state from before a change nobody told the engine about.
    ///
    /// # Errors
    /// Whatever `f` returns, or [`CoralError::Protocol`] if a joined result has the wrong
    /// type, which would mean two different reads share a [`ReadKey`].
    pub async fn read<T, F, Fut>(&self, key: ReadKey, f: F) -> Result<Arc<T>, CoralError>
    where
        T: Send + Sync + 'static,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, CoralError>>,
    {
        let tx = match self.claim(&key) {
            Claim::Join(mut rx) => {
                // The sender fills the slot exactly once, so at most one wait is needed.
                if rx.borrow().is_none() {
                    let _ = rx.changed().await;
                }
                // CoralError is not Clone, so a joined result is copied through clone_result.
                let held = rx.borrow().as_ref().map(clone_result);
                if let Some(result) = held {
                    return downcast(result);
                }
                // The runner vanished without publishing; fall through and do the work.
                return self.run(key, f).await;
            }
            Claim::Run(tx) => tx,
        };
        self.finish(key, tx, f).await
    }

    /// Runs a read that refreshes `.git/index` — status, diff, ls-files.
    ///
    /// Held against mutations, so a write never observes a half-updated index and a read never
    /// races git's own index rewrite.
    ///
    /// # Errors
    /// Whatever `f` returns.
    pub async fn read_index<T, F, Fut>(&self, key: ReadKey, f: F) -> Result<Arc<T>, CoralError>
    where
        T: Send + Sync + 'static,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, CoralError>>,
    {
        let _guard = self.index_gate.read().await;
        self.read(key, f).await
    }

    /// Runs a mutation, serialized against every other write and against index-touching reads.
    ///
    /// The epoch is bumped before the operation as well as after: anything already in flight is
    /// answering a question about a repository that is about to change, so it must not be
    /// joined by a caller arriving later.
    ///
    /// # Errors
    /// Whatever `f` returns.
    pub async fn write<T, F, Fut>(&self, changed: RepoChanged, f: F) -> Result<T, CoralError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, CoralError>>,
    {
        let guard = self.index_gate.write().await;
        self.epoch.fetch_add(1, Ordering::SeqCst);
        let result = f().await;
        // The declared effects are announced whether or not the operation succeeded: a failed
        // merge still leaves MERGE_HEAD and a partly-updated index behind.
        drop(guard);
        self.observe(changed);
        result
    }

    /// Takes the slot for `key`, or a handle to the run already occupying it.
    ///
    /// The lookup and the insert happen under one lock. Doing them separately lets two callers
    /// both find the slot empty and both run, which defeats the whole point: fifty status
    /// calls would spawn fifty git children.
    fn claim(&self, key: &ReadKey) -> Claim {
        let epoch = self.epoch();
        let mut map = self
            .inflight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if let Some(entry) = map.get(key)
            && entry.epoch_started >= epoch
        {
            return Claim::Join(entry.result.clone());
        }

        let (tx, rx) = tokio::sync::watch::channel(None);
        map.insert(
            key.clone(),
            Inflight {
                epoch_started: epoch,
                result: rx,
            },
        );
        Claim::Run(tx)
    }

    /// Runs `f` without claiming a slot, for the rare case where a publisher disappeared.
    async fn run<T, F, Fut>(&self, key: ReadKey, f: F) -> Result<Arc<T>, CoralError>
    where
        T: Send + Sync + 'static,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, CoralError>>,
    {
        let (tx, _rx) = tokio::sync::watch::channel(None);
        self.finish(key, tx, f).await
    }

    /// Executes the read and publishes its outcome to anyone who joined.
    async fn finish<T, F, Fut>(
        &self,
        key: ReadKey,
        tx: tokio::sync::watch::Sender<Option<AnyResult>>,
        f: F,
    ) -> Result<Arc<T>, CoralError>
    where
        T: Send + Sync + 'static,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, CoralError>>,
    {
        let outcome = {
            let _permit = self.children.acquire().await;
            f().await
        };
        self.inflight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&key);

        let shared: AnyResult = match outcome {
            Ok(v) => Ok(Arc::new(v) as Arc<dyn std::any::Any + Send + Sync>),
            Err(e) => Err(e),
        };
        let _ = tx.send(Some(clone_result(&shared)));
        downcast(shared)
    }
}

/// Errors are not `Clone`, so a joined failure is reported by code rather than by value.
fn clone_result(r: &AnyResult) -> AnyResult {
    match r {
        Ok(v) => Ok(Arc::clone(v)),
        Err(e) => Err(CoralError::Protocol {
            label: "engine",
            detail: format!("a joined read failed: {}", e.code()),
        }),
    }
}

fn downcast<T: Send + Sync + 'static>(r: AnyResult) -> Result<Arc<T>, CoralError> {
    r.and_then(|v| {
        v.downcast::<T>().map_err(|_| CoralError::Protocol {
            label: "engine",
            detail: "two reads share a ReadKey but return different types".to_owned(),
        })
    })
}
