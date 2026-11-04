//! Hosts whose kind was declared rather than detected: a self-hosted Gitea or Forgejo, which
//! can be called anything and so cannot be told from its name.
//!
//! Kept in `hosts.json` beside the window's other settings, keyed by host name, and read from
//! disk on every lookup: the credential helper is a process of its own, started by git, and
//! the window may have declared one a moment ago.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::provider::HostKind;

/// Where the declarations are. Tauri's config directory is `dirs::config_dir()` joined with
/// the application's identifier; `CORAL_HOSTS_FILE` overrides it, for tests.
#[must_use]
pub fn file() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("CORAL_HOSTS_FILE") {
        return Some(PathBuf::from(path));
    }
    Some(dirs::config_dir()?.join("dev.coral.app").join("hosts.json"))
}

/// Every declaration, by host name. Empty when there are none or the file cannot be read.
#[must_use]
pub fn all() -> BTreeMap<String, HostKind> {
    file()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// The kind declared for a host name, if one was.
#[must_use]
pub fn kind_of(hostname: &str) -> Option<HostKind> {
    all().get(&hostname.to_ascii_lowercase()).copied()
}

/// Declares what kind of host `hostname` is, or forgets it with `None`.
///
/// # Errors
/// An I/O failure writing the file.
pub fn declare(hostname: &str, kind: Option<HostKind>) -> std::io::Result<()> {
    let Some(path) = file() else {
        return Err(std::io::Error::other("there is no configuration directory"));
    };
    let mut held = all();
    match kind {
        Some(kind) => held.insert(hostname.to_ascii_lowercase(), kind),
        None => held.remove(&hostname.to_ascii_lowercase()),
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(&held)?)
}
