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

/// One private key on disk, found in `~/.ssh` or chosen from anywhere.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct SshKey {
    /// The private key's path, which is what `core.sshCommand` names.
    pub path: String,
    /// The public half, when it is beside the private one.
    pub public_path: Option<String>,
    /// The comment at the end of the public key, usually `user@host`.
    pub comment: String,
    /// `ssh-ed25519`, `ssh-rsa`, and so on.
    pub kind: String,
    /// Readable by others, which OpenSSH refuses to use ("UNPROTECTED PRIVATE KEY FILE").
    pub too_open: bool,
}

/// Builds the `core.sshCommand` that pins git to one key.
///
/// Both flags are load-bearing, and neither is enough on its own.
///
/// `IdentitiesOnly=yes` stops ssh offering every key the agent holds before the one it was
/// asked for. `-F none` stops the user's own `~/.ssh/config` adding a second key beside it:
/// `-i` and a host block's `IdentityFile` accumulate into one list rather than the first
/// winning, and the agent then reorders that list into whatever order it happens to hold the
/// keys in. Measured against a real host: with a `Host gitlab.com` block naming another key,
/// `ssh -i chosen -o IdentitiesOnly=yes` offered the config's key first and authenticated as
/// the wrong account, silently and every time. That is the exact failure pinning a key exists
/// to prevent, so a pin that cannot override the file has not pinned anything.
///
/// The cost is that a pinned repository does not read `~/.ssh/config` at all — no `ProxyJump`,
/// no per-host `Port`, no `HostName` alias — which is why nothing is written unless a key was
/// actually chosen, and why the interface says so where the choice is made. `none` rather than
/// `/dev/null` because there is no such device on Windows; OpenSSH has understood it since 8.4.
#[must_use]
pub fn command_for(private_key: &str) -> String {
    let home = dirs::home_dir();
    let path = written_path(private_key, home.as_deref(), cfg!(windows));
    format!(
        "ssh -F none -i {} -o IdentitiesOnly=yes",
        crate::process::shell_word(&path)
    )
}

/// The key's path as `core.sshCommand` should spell it.
///
/// A leading `~` is expanded here because the single quotes `shell_word` adds stop the shell
/// doing it. On Windows the separators become `/`: git runs the command through its own `sh`,
/// and both that and the ssh it bundles read `C:/Users/...`, while a backslash is one more
/// thing for a hand-edited command to get wrong.
#[must_use]
pub fn written_path(key: &str, home: Option<&Path>, windows: bool) -> String {
    let expanded = match (key.strip_prefix('~'), home) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with(['/', '\\']) => {
            format!("{}{rest}", home.display())
        }
        _ => key.to_owned(),
    };
    if windows {
        expanded.replace('\\', "/")
    } else {
        expanded
    }
}

/// Reads the key back out of a `core.sshCommand`, or `None` when it names none.
#[must_use]
pub fn key_in_command(command: &str) -> Option<String> {
    let rest = command.split("-i").nth(1)?.trim_start();
    let key = match rest.as_bytes().first()? {
        b'\'' => unquoted(&rest[1..]),
        b'"' => rest[1..].split('"').next().unwrap_or_default().to_owned(),
        _ => rest.split(' ').next().unwrap_or_default().to_owned(),
    };
    let key = key.trim();
    if key.is_empty() {
        None
    } else {
        Some(key.to_owned())
    }
}

/// A single-quoted shell word, back to the path it spells.
///
/// An apostrophe inside one is written `'\''` — quoting closed, escaped, opened again — so a
/// quote is the end of the word only when that is not what follows it.
fn unquoted(mut rest: &str) -> String {
    let mut out = String::new();
    loop {
        let Some(at) = rest.find('\'') else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..at]);
        match rest[at..].strip_prefix(r"'\''") {
            Some(after) => {
                out.push('\'');
                rest = after;
            }
            None => return out,
        }
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
        let (kind, comment) = described_by(&path);
        keys.push(SshKey {
            path: private.display().to_string(),
            public_path: Some(path.display().to_string()),
            comment,
            kind,
            too_open: too_open(&private),
        });
    }
    keys.sort_by(|a, b| a.path.cmp(&b.path));
    keys
}

/// The kind and comment a public key file states. The body between them is skipped.
fn described_by(public: &Path) -> (String, String) {
    let text = std::fs::read_to_string(public).unwrap_or_default();
    let mut parts = text.split_whitespace();
    let kind = parts.next().unwrap_or_default().to_owned();
    let comment = parts.nth(1).unwrap_or_default().to_owned();
    (kind, comment)
}

/// Describes one file the user chose as their private key, wherever it is.
///
/// Only the first line of the private file is read, to tell a key from anything else; the
/// kind and comment come from the `.pub` beside it when there is one. Nothing runs
/// `ssh-keygen -y`, which would stop to ask for the passphrase of an encrypted key.
///
/// # Errors
/// [`CoralError::Refused`] saying what is wrong with the file: missing, the public half with no
/// private one beside it, a `PuTTY` key, or not a private key at all.
pub fn key_at(chosen: &Path) -> Result<SshKey, CoralError> {
    let refuse = |detail: String| CoralError::Refused {
        label: "ssh",
        detail,
    };
    // A typed `~/.ssh/...` means what it would at a shell, and what `command_for` will write.
    let home = dirs::home_dir();
    let chosen = PathBuf::from(written_path(
        &chosen.to_string_lossy(),
        home.as_deref(),
        false,
    ));
    let private = private_half(&chosen).map_err(refuse)?;
    let head = first_line(&private).map_err(refuse)?;
    let kind_from_header = header_kind(&head).ok_or_else(|| {
        refuse(if head.starts_with("PuTTY-User-Key-File") {
            format!(
                "{} is a PuTTY key. Open it in PuTTYgen and use Conversions → Export OpenSSH key, \
                 then choose the exported file",
                private.display()
            )
        } else {
            format!("{} is not a private key", private.display())
        })
    })?;

    let public = sibling_pub(&private);
    let (kind, comment) = match public.as_deref() {
        Some(public) => described_by(public),
        None => (kind_from_header.to_owned(), String::new()),
    };
    Ok(SshKey {
        path: private.display().to_string(),
        public_path: public.map(|p| p.display().to_string()),
        comment,
        kind,
        too_open: too_open(&private),
    })
}

/// The private file for a choice, forgiving the common slip of picking `key.pub`.
fn private_half(chosen: &Path) -> Result<PathBuf, String> {
    if !chosen.is_file() {
        return Err(format!("{} does not exist", chosen.display()));
    }
    if chosen.extension().and_then(|e| e.to_str()) != Some("pub") {
        return Ok(chosen.to_path_buf());
    }
    let private = chosen.with_extension("");
    if private.is_file() {
        Ok(private)
    } else {
        Err(format!(
            "{} is the public half of a key; choose the private file it was made with",
            chosen.display()
        ))
    }
}

fn first_line(path: &Path) -> Result<String, String> {
    use std::io::Read;
    let mut head = Vec::with_capacity(256);
    std::fs::File::open(path)
        .and_then(|f| f.take(4096).read_to_end(&mut head))
        .map_err(|e| format!("{} could not be read: {e}", path.display()))?;
    let text = String::from_utf8_lossy(&head);
    Ok(text
        .trim_start_matches('\u{feff}')
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned())
}

/// The kind a PEM header names, or `None` when the line is not a private key's header.
fn header_kind(line: &str) -> Option<&'static str> {
    let label = line.strip_prefix("-----BEGIN ")?.strip_suffix("-----")?;
    match label {
        "OPENSSH PRIVATE KEY" | "PRIVATE KEY" | "ENCRYPTED PRIVATE KEY" => Some("private key"),
        "RSA PRIVATE KEY" => Some("ssh-rsa"),
        "EC PRIVATE KEY" => Some("ecdsa"),
        "DSA PRIVATE KEY" => Some("ssh-dss"),
        _ => None,
    }
}

fn sibling_pub(private: &Path) -> Option<PathBuf> {
    let mut name = private.file_name()?.to_os_string();
    name.push(".pub");
    let public = private.with_file_name(name);
    public.is_file().then_some(public)
}

#[cfg(unix)]
fn too_open(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o077 != 0)
}

/// Windows has no mode bits; Git for Windows' ssh does not check the file's ACL either.
#[cfg(not(unix))]
fn too_open(_: &Path) -> bool {
    false
}

/// Where ssh keys live for this user: `~/.ssh`, or `%USERPROFILE%\.ssh` on Windows, where
/// `HOME` is usually not set at all.
#[must_use]
pub fn default_directory() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".ssh")
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

    // Hidden the same way every git child is; on Windows a console subsystem child opens a
    // window whether or not it writes to it.
    let mut built = std::process::Command::new("ssh-keygen");
    built
        .args(["-t", "ed25519", "-N", passphrase, "-C", comment, "-f"])
        .arg(path);
    crate::process::hide_console(&mut built);

    let out = tokio::process::Command::from(built)
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
        public_path: Some(public.display().to_string()),
        kind: parts.next().unwrap_or("ssh-ed25519").to_owned(),
        comment: parts.nth(1).unwrap_or(comment).to_owned(),
        too_open: false,
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
