use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::{Semaphore, broadcast};

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
    epoch: AtomicU64,
    events: broadcast::Sender<RepoEvent>,
}

type AnyResult = Result<Arc<dyn std::any::Any + Send + Sync>, CoralError>;

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
    /// # Errors
    /// Whatever `f` returns, or [`CoralError::Protocol`] if a joined result has the wrong
    /// type, which would mean two different reads share a [`ReadKey`].
    pub async fn read<T, F, Fut>(&self, key: ReadKey, f: F) -> Result<Arc<T>, CoralError>
    where
        T: Send + Sync + 'static,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, CoralError>>,
    {
        if let Some(mut rx) = self.join(&key) {
            // The sender fills the slot exactly once, so at most one wait is needed.
            if rx.borrow().is_none() {
                let _ = rx.changed().await;
            }
            // CoralError is not Clone, so a joined result is copied through clone_result.
            let held = rx.borrow().as_ref().map(clone_result);
            if let Some(result) = held {
                return downcast(result);
            }
        }

        let (tx, rx) = tokio::sync::watch::channel(None);
        let started = self.epoch();
        self.register(key.clone(), started, rx);

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

    /// An in-flight read worth joining: same question, and not started before a change we
    /// already know about.
    fn join(&self, key: &ReadKey) -> Option<tokio::sync::watch::Receiver<Option<AnyResult>>> {
        let map = self
            .inflight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let entry = map.get(key)?;
        (entry.epoch_started >= self.epoch()).then(|| entry.result.clone())
    }

    fn register(
        &self,
        key: ReadKey,
        epoch_started: u64,
        result: tokio::sync::watch::Receiver<Option<AnyResult>>,
    ) {
        self.inflight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                key,
                Inflight {
                    epoch_started,
                    result,
                },
            );
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
