//! How git reaches a server over ssh, layered per repository the way signing is.
//!
//! An ssh key belongs to a repository rather than to a person, for the same reason a signing
//! key does: the same user pushes to work with a company key and to their own account with
//! another. The app level is a default and the repository overrides it, which is exactly how
//! git's own configuration is layered — so the command line and Coral agree.

use std::path::{Path, PathBuf};

use crate::config::{AppConfig, Scope};
use crate::error::CoralError;
use crate::process::GitRunner;
use crate::repo::RepoLocation;

/// How git is told to reach an ssh server.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct SshConfig {
    /// True when nothing overrides `ssh`, so the running agent decides which key to offer.
    pub use_agent: bool,
    /// The private key git is pointed at, when it is pointed at one.
    pub private_key: String,
    /// Its public half. git never needs this; it is what gets uploaded to the host.
    pub public_key: String,
    /// `core.sshCommand`, verbatim, since a user may have written one by hand.
    pub command: String,
    /// `credential.helper`, so the screen can say which one is answering.
    pub credential_helper: String,
}

/// What this repository sets for itself. `None` means it inherits.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct SshOverrides {
    /// The private key to use here. Setting it writes `core.sshCommand`.
    pub private_key: Option<String>,
    pub public_key: Option<String>,
    pub credential_helper: Option<String>,
}

impl SshOverrides {
    /// True when this repository overrides nothing and simply inherits.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.private_key.is_none() && self.public_key.is_none() && self.credential_helper.is_none()
    }
}

/// Ssh as it stands for one repository: what will happen, what it inherits, what it sets.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct SshScopes {
    pub effective: SshConfig,
    pub global: SshConfig,
    pub local: SshOverrides,
}

/// One key pair found on disk.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct SshKey {
    /// The private key's path, which is what `core.sshCommand` names.
    pub path: String,
    /// The public half, when it is beside the private one.
    pub public_path: String,
    /// The comment at the end of the public key, usually `user@host`.
    pub comment: String,
    /// `ssh-ed25519`, `ssh-rsa`, and so on.
    pub kind: String,
}

/// Builds the `core.sshCommand` that pins git to one key.
///
/// `IdentitiesOnly=yes` is not optional. Without it ssh offers every key the agent holds
/// before the one it was asked for, and a server that accepts one of those authenticates as
/// the wrong account — which is the exact failure a per-repository key exists to avoid.
#[must_use]
pub fn command_for(private_key: &str) -> String {
    format!("ssh -i '{private_key}' -o IdentitiesOnly=yes")
}

/// Reads the key back out of a `core.sshCommand`, or `None` when it names none.
#[must_use]
pub fn key_in_command(command: &str) -> Option<String> {
    let rest = command.split("-i").nth(1)?.trim_start();
    let (quote, rest) = match rest.as_bytes().first()? {
        b'\'' => ('\'', &rest[1..]),
        b'"' => ('"', &rest[1..]),
        _ => (' ', rest),
    };
    let end = rest.find(quote).unwrap_or(rest.len());
    let key = rest[..end].trim();
    if key.is_empty() {
        None
    } else {
        Some(key.to_owned())
    }
}

/// Every key pair in a directory, usually `~/.ssh`.
///
/// Paired by name: a private key is the file whose `.pub` sits beside it. Reading the private
/// halves is deliberately avoided — the public file says everything the screen shows, and
/// nothing here should be opening a secret it does not need.
#[must_use]
pub fn keys_in(dir: &Path) -> Vec<SshKey> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut keys = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("pub") {
            continue;
        }
        let private = path.with_extension("");
        if !private.is_file() {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let mut parts = text.split_whitespace();
        let kind = parts.next().unwrap_or_default().to_owned();
        // The body is skipped; the comment is whatever follows it, and may be absent.
        let comment = parts.nth(1).unwrap_or_default().to_owned();
        keys.push(SshKey {
            path: private.display().to_string(),
            public_path: path.display().to_string(),
            comment,
            kind,
        });
    }
    keys.sort_by(|a, b| a.path.cmp(&b.path));
    keys
}

/// Where ssh keys live for this user.
#[must_use]
pub fn default_directory() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join(".ssh")
}

/// Creates an ed25519 key pair at `path`.
///
/// ed25519 rather than RSA: it is the default `ssh-keygen` itself now offers, every host Coral
/// speaks to accepts it, and the key is short enough to read. The passphrase may be empty,
/// which is the user's decision to make.
///
/// # Errors
/// [`CoralError::Refused`] with `ssh-keygen`'s own message, which explains a path that already
/// exists or a directory that cannot be written far better than anything invented here.
pub async fn generate(path: &Path, comment: &str, passphrase: &str) -> Result<SshKey, CoralError> {
    if path.exists() {
        return Err(CoralError::Refused {
            label: "ssh",
            detail: format!("{} already exists; choose another name", path.display()),
        });
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }

    let out = tokio::process::Command::new("ssh-keygen")
        .args(["-t", "ed25519", "-N", passphrase, "-C", comment, "-f"])
        .arg(path)
        .output()
        .await
        .map_err(|e| CoralError::Refused {
            label: "ssh",
            detail: format!("could not run ssh-keygen: {e}"),
        })?;

    if !out.status.success() {
        return Err(CoralError::Refused {
            label: "ssh",
            detail: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        });
    }

    let public = path.with_extension("pub");
    let text = std::fs::read_to_string(&public).unwrap_or_default();
    let mut parts = text.split_whitespace();
    Ok(SshKey {
        path: path.display().to_string(),
        public_path: public.display().to_string(),
        kind: parts.next().unwrap_or("ssh-ed25519").to_owned(),
        comment: parts.nth(1).unwrap_or(comment).to_owned(),
    })
}

impl RepoLocation {
    /// Reads the ssh settings at every level.
    ///
    /// # Errors
    /// Propagates git failures other than the absent key `git config` reports with exit 1.
    pub async fn ssh_scopes(
        &self,
        runner: &GitRunner,
        app: AppConfig<'_>,
    ) -> Result<SshScopes, CoralError> {
        let effective = self.read_ssh(runner, Scope::Effective(app)).await?;
        let global = self.read_ssh(runner, Scope::App(app)).await?;

        let at = Scope::Repository;
        let local_command = self.config_get(runner, at, "core.sshCommand").await?;
        let local = SshOverrides {
            private_key: local_command.as_deref().and_then(key_in_command),
            public_key: self.config_get(runner, at, "coral.sshPublicKey").await?,
            credential_helper: self.config_get(runner, at, "credential.helper").await?,
        };

        Ok(SshScopes {
            effective,
            global,
            local,
        })
    }

    /// Sets the app-level defaults every repository inherits.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn set_ssh_global(
        &self,
        runner: &GitRunner,
        app: AppConfig<'_>,
        config: &SshConfig,
    ) -> Result<(), CoralError> {
        // Using the agent means saying nothing, not saying something else: an empty
        // `core.sshCommand` would still shadow whatever the user set by hand.
        let at = Scope::App(app);
        let command = if config.use_agent || config.private_key.is_empty() {
            None
        } else {
            Some(command_for(&config.private_key))
        };
        self.config_put(runner, at, "core.sshCommand", command.as_deref())
            .await?;
        self.config_put(
            runner,
            at,
            "coral.sshPublicKey",
            none_if_empty(&config.public_key),
        )
        .await?;
        self.config_put(
            runner,
            at,
            "credential.helper",
            none_if_empty(&config.credential_helper),
        )
        .await
    }

    /// Sets what this repository overrides. A `None` field clears the override.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn set_ssh_local(
        &self,
        runner: &GitRunner,
        overrides: &SshOverrides,
    ) -> Result<(), CoralError> {
        let at = Scope::Repository;
        let command = overrides
            .private_key
            .as_deref()
            .filter(|k| !k.is_empty())
            .map(command_for);
        self.config_put(runner, at, "core.sshCommand", command.as_deref())
            .await?;
        self.config_put(
            runner,
            at,
            "coral.sshPublicKey",
            overrides.public_key.as_deref(),
        )
        .await?;
        self.config_put(
            runner,
            at,
            "credential.helper",
            overrides.credential_helper.as_deref(),
        )
        .await
    }

    async fn read_ssh(&self, runner: &GitRunner, at: Scope<'_>) -> Result<SshConfig, CoralError> {
        let command = self
            .config_get(runner, at, "core.sshCommand")
            .await?
            .unwrap_or_default();
        let private_key = key_in_command(&command).unwrap_or_default();
        Ok(SshConfig {
            use_agent: private_key.is_empty(),
            private_key,
            public_key: self
                .config_get(runner, at, "coral.sshPublicKey")
                .await?
                .unwrap_or_default(),
            command,
            credential_helper: self
                .config_get(runner, at, "credential.helper")
                .await?
                .unwrap_or_default(),
        })
    }
}

fn none_if_empty(value: &str) -> Option<&str> {
    if value.is_empty() { None } else { Some(value) }
}
