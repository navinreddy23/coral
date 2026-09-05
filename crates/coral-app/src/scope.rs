//! What one repository is showing in the graph, and what it is not.
//!
//! Not the session, which holds which repositories are open: this is about one repository's
//! own history and stays true of it whichever window it is opened in. Kept beside the session
//! all the same, since both are about how a person arranged their work.

// Tauri requires `State` by value in a command signature; it is a handle, not the data.
#![allow(clippy::needless_pass_by_value)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use coral_core::graph::Tips;
use coral_core::refs::GitRef;

use crate::commands::IpcError;

/// Which refs a repository's graph is walked from.
///
/// Solo and hidden are held together rather than as one list, because leaving solo has to give
/// the hidden branches back. Collapsing them into the walk's own vocabulary too early is what
/// would lose them: [`Tips`] can say "only this one" or "all but these", never both, and the
/// user who soloed a branch still expects the spike they hid last week to stay hidden.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoScope {
    /// The one ref being soloed, by full name.
    #[serde(default)]
    pub solo: Option<String>,
    /// Refs taken out of the walk, by full name.
    #[serde(default)]
    pub hidden: Vec<String>,
}

impl RepoScope {
    /// Where the walk starts.
    ///
    /// Solo wins outright. It is a mode the interface announces and offers a way out of, while
    /// hiding is a standing preference, so the two cannot both be in force without the window
    /// having to explain which is winning.
    #[must_use]
    pub fn tips(&self) -> Tips {
        match &self.solo {
            Some(name) => Tips::Only(vec![name.clone()]),
            None if self.hidden.is_empty() => Tips::All,
            None => Tips::Except(self.hidden.clone()),
        }
    }

    /// Whether this is the plain full graph, so the common case can skip a rewalk.
    #[must_use]
    pub fn is_everything(&self) -> bool {
        self.solo.is_none() && self.hidden.is_empty()
    }
}

/// The same scope with every name the repository no longer has taken out.
///
/// A branch soloed yesterday and deleted this morning would otherwise walk from nothing and
/// paint an empty graph, which reads as the application having broken rather than as a branch
/// having gone. Hidden names are pruned for the plainer reason that they accumulate: a
/// repository whose branches are deleted and recreated would carry a list nothing ever
/// shortens.
#[must_use]
pub fn pruned(scope: &RepoScope, refs: &[GitRef]) -> RepoScope {
    let known: std::collections::HashSet<&str> = refs.iter().map(|r| r.name.as_str()).collect();
    RepoScope {
        solo: scope.solo.clone().filter(|n| known.contains(n.as_str())),
        hidden: scope
            .hidden
            .iter()
            .filter(|n| known.contains(n.as_str()))
            .cloned()
            .collect(),
    }
}

/// Every repository's scope, and where they are persisted.
pub struct Scopes {
    inner: Mutex<HashMap<String, RepoScope>>,
    path: PathBuf,
}

impl Scopes {
    #[must_use]
    pub fn load(path: PathBuf) -> Self {
        let stored: HashMap<String, RepoScope> = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            inner: Mutex::new(stored),
            path,
        }
    }

    /// What `repo` is showing. The full graph for one nobody has narrowed.
    #[must_use]
    pub fn read(&self, repo: &str) -> RepoScope {
        self.held().get(repo).cloned().unwrap_or_default()
    }

    /// Records what `repo` is showing, forgetting the entry entirely when it is everything.
    ///
    /// Dropping it rather than storing an empty one keeps the file to the repositories somebody
    /// actually narrowed, which is what makes it readable by hand.
    pub fn set(&self, repo: &str, scope: RepoScope) {
        let mut held = self.held();
        if scope.is_everything() {
            held.remove(repo);
        } else {
            held.insert(repo.to_owned(), scope);
        }
        if let Err(e) = save(&self.path, &held) {
            tracing::warn!(error = %e, path = %self.path.display(), "could not save the scopes");
        }
    }

    fn held(&self) -> std::sync::MutexGuard<'_, HashMap<String, RepoScope>> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn save(path: &Path, scopes: &HashMap<String, RepoScope>) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(scopes)?)
}

/// Reads the refs once, so a scope can be pruned against what the repository actually has.
async fn live_refs(path: &str) -> Result<Vec<GitRef>, IpcError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc = coral_core::repo::RepoLocation::discover(&runner, Path::new(path)).await?;
    Ok(loc.refs(&runner).await?)
}

/// What this repository is showing, with names it no longer has taken out.
///
/// # Errors
/// Propagates git failures from reading the refs.
#[tauri::command]
pub async fn graph_scope(
    scopes: tauri::State<'_, Scopes>,
    path: String,
) -> Result<RepoScope, IpcError> {
    let held = scopes.read(&path);
    if held.is_everything() {
        return Ok(held);
    }
    let live = pruned(&held, &live_refs(&path).await?);
    if live != held {
        scopes.set(&path, live.clone());
    }
    Ok(live)
}

/// Sets what this repository shows, and answers with what was actually stored.
///
/// The answer is the pruned scope rather than nothing, so the window cannot come away believing
/// it soloed a branch that has since been deleted.
///
/// # Errors
/// Propagates git failures from reading the refs.
#[tauri::command]
pub async fn set_graph_scope(
    scopes: tauri::State<'_, Scopes>,
    path: String,
    scope: RepoScope,
) -> Result<RepoScope, IpcError> {
    let live = if scope.is_everything() {
        scope
    } else {
        pruned(&scope, &live_refs(&path).await?)
    };
    scopes.set(&path, live.clone());
    Ok(live)
}
