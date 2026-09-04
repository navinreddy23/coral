//! Commit signing: which key, which program, and whether to sign at all.
//!
//! Coral configures git and gets out of the way. Signing itself is done by git, through the
//! program named in `gpg.program`, so a commit made here is signed exactly as one made at a
//! terminal would be — the same agent, the same pinentry, the same trust decisions.

use std::path::Path;

use secrecy::{ExposeSecret as _, SecretString};

pub use crate::config::AppConfig;

use crate::config::Scope;
use crate::error::CoralError;
use crate::process::GitRunner;
use crate::repo::RepoLocation;

/// The signature format git will produce.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "lowercase")]
pub enum SigningFormat {
    #[default]
    OpenPgp,
    X509,
    Ssh,
}

impl SigningFormat {
    /// The value git stores in `gpg.format`. `openpgp` is also its default when unset.
    #[must_use]
    pub const fn config_value(self) -> &'static str {
        match self {
            Self::OpenPgp => "openpgp",
            Self::X509 => "x509",
            Self::Ssh => "ssh",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "x509" => Self::X509,
            "ssh" => Self::Ssh,
            _ => Self::OpenPgp,
        }
    }

    /// The program git runs for this format when none is configured.
    #[must_use]
    pub const fn default_program(self) -> &'static str {
        match self {
            Self::OpenPgp => "gpg",
            Self::X509 => "gpgsm",
            Self::Ssh => "ssh-keygen",
        }
    }
}

/// Everything the signing settings screen shows.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct SigningConfig {
    pub format: SigningFormat,
    /// Empty when git's default for the format is in use.
    pub program: String,
    /// `user.signingkey`. Empty when git is left to choose.
    pub key: String,
    pub sign_commits: bool,
    pub sign_tags: bool,
}

/// A key that could sign.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct SigningKey {
    /// What goes in `user.signingkey`: a full fingerprint, or a path for ssh.
    pub id: String,
    /// Who the key belongs to, for the list.
    pub label: String,
    /// Seconds since the epoch, or `None` for a key that does not expire.
    pub expires: Option<i64>,
    /// True when it has expired already, which is why it is still listed rather than dropped.
    pub expired: bool,
}

/// What this repository sets for itself.
///
/// `None` means the repository says nothing and the app-level setting applies. That is a
/// different state from a value that happens to match, and the difference is what lets a
/// setting be cleared back to inheriting rather than pinned to whatever it inherited.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct SigningOverrides {
    pub format: Option<SigningFormat>,
    pub program: Option<String>,
    pub key: Option<String>,
    pub sign_commits: Option<bool>,
    pub sign_tags: Option<bool>,
}

impl SigningOverrides {
    /// True when this repository overrides nothing and simply inherits.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.format.is_none()
            && self.program.is_none()
            && self.key.is_none()
            && self.sign_commits.is_none()
            && self.sign_tags.is_none()
    }
}

/// Signing as it stands for one repository: what will happen, what it inherits, what it sets.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct SigningScopes {
    /// What git will actually do here.
    pub effective: SigningConfig,
    /// The app-level defaults, which every repository inherits until it says otherwise.
    pub global: SigningConfig,
    /// What this repository sets for itself.
    pub local: SigningOverrides,
}

impl RepoLocation {
    /// Reads signing at every level.
    ///
    /// A key belongs to a repository, not to a person: the same user signs work commits with a
    /// company key and their own with another. So the app-level setting is a default and the
    /// repository overrides it, which is exactly how git's own configuration is layered —
    /// using it rather than a private store means the command line agrees with Coral.
    ///
    /// # Errors
    /// Propagates git failures other than the absent key `git config` reports with exit 1.
    pub async fn signing_scopes(
        &self,
        runner: &GitRunner,
        app: AppConfig<'_>,
    ) -> Result<SigningScopes, CoralError> {
        let effective = self.read_signing(runner, Scope::Effective(app)).await?;
        let global = self.read_signing(runner, Scope::App(app)).await?;

        let format = effective.format;
        let program_key = format!("gpg.{}.program", format.config_value());
        let local = SigningOverrides {
            format: self
                .config_get(runner, Scope::Repository, "gpg.format")
                .await?
                .map(|v| SigningFormat::parse(&v)),
            program: self
                .config_get(runner, Scope::Repository, &program_key)
                .await?,
            key: self
                .config_get(runner, Scope::Repository, "user.signingkey")
                .await?,
            sign_commits: self
                .config_get(runner, Scope::Repository, "commit.gpgsign")
                .await?
                .map(|v| is_true(&v)),
            sign_tags: self
                .config_get(runner, Scope::Repository, "tag.gpgSign")
                .await?
                .map(|v| is_true(&v)),
        };

        Ok(SigningScopes {
            effective,
            global,
            local,
        })
    }

    /// Sets the app-level defaults every repository inherits.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn set_signing_global(
        &self,
        runner: &GitRunner,
        app: AppConfig<'_>,
        config: &SigningConfig,
    ) -> Result<(), CoralError> {
        let at = Scope::App(app);
        let program_key = format!("gpg.{}.program", config.format.config_value());
        self.config_put(runner, at, "gpg.format", Some(config.format.config_value()))
            .await?;
        self.config_put(runner, at, &program_key, some_unless_empty(&config.program))
            .await?;
        self.config_put(
            runner,
            at,
            "user.signingkey",
            some_unless_empty(&config.key),
        )
        .await?;
        self.config_put(
            runner,
            at,
            "commit.gpgsign",
            Some(bool_str(config.sign_commits)),
        )
        .await?;
        self.config_put(runner, at, "tag.gpgSign", Some(bool_str(config.sign_tags)))
            .await
    }

    /// Sets what this repository overrides. A `None` field clears the override.
    ///
    /// `format` says which program key the override applies to, since git stores the program
    /// per format rather than once.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn set_signing_local(
        &self,
        runner: &GitRunner,
        overrides: &SigningOverrides,
        format: SigningFormat,
    ) -> Result<(), CoralError> {
        let at = Scope::Repository;
        let program_key = format!("gpg.{}.program", format.config_value());
        self.config_put(
            runner,
            at,
            "gpg.format",
            overrides.format.map(SigningFormat::config_value),
        )
        .await?;
        self.config_put(runner, at, &program_key, overrides.program.as_deref())
            .await?;
        self.config_put(runner, at, "user.signingkey", overrides.key.as_deref())
            .await?;
        self.config_put(
            runner,
            at,
            "commit.gpgsign",
            overrides.sign_commits.map(bool_str),
        )
        .await?;
        self.config_put(runner, at, "tag.gpgSign", overrides.sign_tags.map(bool_str))
            .await
    }

    /// One configuration value as it applies here, or empty when unset.
    ///
    /// # Errors
    /// Propagates git failures other than the absent key.
    pub async fn config_value(&self, runner: &GitRunner, key: &str) -> Result<String, CoralError> {
        Ok(self
            .config_get(runner, Scope::Effective(AppConfig(None)), key)
            .await?
            .unwrap_or_default())
    }

    async fn read_signing(
        &self,
        runner: &GitRunner,
        scope: Scope<'_>,
    ) -> Result<SigningConfig, CoralError> {
        let format = SigningFormat::parse(
            &self
                .config_get(runner, scope, "gpg.format")
                .await?
                .unwrap_or_default(),
        );
        // Per-format overrides win: `gpg.ssh.program` is how ssh signing is pointed at a
        // different binary, and reading only `gpg.program` would report the wrong one.
        let per_format = format!("gpg.{}.program", format.config_value());
        let mut program = self.config_get(runner, scope, &per_format).await?;
        if program.is_none() && format == SigningFormat::OpenPgp {
            program = self.config_get(runner, scope, "gpg.program").await?;
        }

        Ok(SigningConfig {
            format,
            program: program.unwrap_or_default(),
            key: self
                .config_get(runner, scope, "user.signingkey")
                .await?
                .unwrap_or_default(),
            sign_commits: self
                .config_get(runner, scope, "commit.gpgsign")
                .await?
                .is_some_and(|v| is_true(&v)),
            sign_tags: self
                .config_get(runner, scope, "tag.gpgSign")
                .await?
                .is_some_and(|v| is_true(&v)),
        })
    }
}

const fn bool_str(on: bool) -> &'static str {
    if on { "true" } else { "false" }
}

fn some_unless_empty(value: &str) -> Option<&str> {
    if value.is_empty() { None } else { Some(value) }
}

fn is_true(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "yes" | "on" | "1"
    )
}

/// Parses `--with-colons` output from gpg or gpgsm into the keys that can sign.
///
/// The format is documented in `GnuPG`'s DETAILS file and is stable, which is the whole reason
/// to ask for it rather than parse the human listing. A `sec` record opens a key; the `fpr`
/// and `uid` records that follow belong to it until the next `sec`.
///
/// Expired keys are kept rather than dropped: a key that has just expired is exactly the one a
/// person is looking for when they come to this screen, and hiding it makes the screen look
/// broken instead of explaining.
#[must_use]
pub fn parse_secret_keys(output: &[u8]) -> Vec<SigningKey> {
    let text = String::from_utf8_lossy(output);
    let mut keys: Vec<SigningKey> = Vec::new();
    let mut open: Option<(SigningKey, bool)> = None;

    for line in text.lines() {
        let f: Vec<&str> = line.split(':').collect();
        match f.first().copied() {
            Some("sec") => {
                if let Some((key, can_sign)) = open.take()
                    && can_sign
                {
                    keys.push(key);
                }
                let validity = f.get(1).copied().unwrap_or_default();
                // Revoked and disabled keys cannot sign at all, so they are not offered.
                if matches!(validity, "r" | "d") {
                    open = None;
                    continue;
                }
                let expires = f
                    .get(6)
                    .and_then(|v| v.parse::<i64>().ok())
                    .filter(|v| *v > 0);
                // Lowercase capabilities describe this key; `s` is signing.
                let can_sign = f.get(11).is_some_and(|c| c.contains('s'));
                open = Some((
                    SigningKey {
                        id: f.get(4).copied().unwrap_or_default().to_owned(),
                        label: String::new(),
                        expires,
                        expired: validity == "e",
                    },
                    can_sign,
                ));
            }
            // The fingerprint is what `user.signingkey` should hold: a key id is only its last
            // 16 hex digits and two keys can share one.
            Some("fpr") => {
                if let Some((key, _)) = open.as_mut()
                    && let Some(fpr) = f.get(9).filter(|v| !v.is_empty())
                {
                    (*fpr).clone_into(&mut key.id);
                }
            }
            Some("uid") => {
                if let Some((key, _)) = open.as_mut()
                    && key.label.is_empty()
                    && let Some(uid) = f.get(9).filter(|v| !v.is_empty())
                {
                    key.label = unescape_colons(uid);
                }
            }
            _ => {}
        }
    }
    if let Some((key, can_sign)) = open
        && can_sign
    {
        keys.push(key);
    }
    keys
}

/// `GnuPG` escapes a colon in a user id as `\x3a`, and a backslash as `\x5c`.
fn unescape_colons(raw: &str) -> String {
    if !raw.contains("\\x") {
        return raw.to_owned();
    }
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let rest: String = chars.clone().take(3).collect();
        if let Some(hex) = rest.strip_prefix('x')
            && let Ok(byte) = u8::from_str_radix(hex, 16)
        {
            out.push(byte as char);
            chars.nth(2);
        } else {
            out.push(c);
        }
    }
    out
}

/// Lists the ssh public keys that could sign.
///
/// git's ssh signing takes a path to a public key, or the key itself, so these are listed by
/// file rather than asked of an agent — an agent-held key still needs a file for git to name.
#[must_use]
pub fn ssh_keys_in(dir: &Path) -> Vec<SigningKey> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut keys: Vec<SigningKey> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "pub"))
        .filter_map(|e| {
            let contents = std::fs::read_to_string(e.path()).ok()?;
            // `<type> <base64> <comment>`; the comment is what identifies it to a person.
            let mut parts = contents.split_whitespace();
            let kind = parts.next()?;
            parts.next()?;
            let comment = parts.next().unwrap_or("");
            Some(SigningKey {
                id: e.path().to_string_lossy().into_owned(),
                label: if comment.is_empty() {
                    kind.to_owned()
                } else {
                    format!("{comment} ({kind})")
                },
                expires: None,
                expired: false,
            })
        })
        .collect();
    keys.sort_by(|a, b| a.id.cmp(&b.id));
    keys
}

/// Runs a signing program and returns its stdout.
///
/// Not [`GitCommand`], which exists to run git and adds git's own arguments to everything. The
/// passphrase goes on stdin: an argument is visible in `ps` to every process on the machine
/// for as long as the program runs.
async fn run(
    program: &str,
    args: &[&str],
    stdin: Option<&SecretString>,
) -> Result<Vec<u8>, CoralError> {
    use tokio::io::AsyncWriteExt as _;

    let mut command = tokio::process::Command::new(program);
    command
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = command.spawn().map_err(|e| CoralError::Protocol {
        label: "signing",
        detail: format!("could not run {program}: {e}"),
    })?;

    if let Some(mut pipe) = child.stdin.take() {
        let secret = stdin
            .map(|s| s.expose_secret().to_owned())
            .unwrap_or_default();
        let _ = pipe.write_all(secret.as_bytes()).await;
        let _ = pipe.shutdown().await;
    }

    let out = child
        .wait_with_output()
        .await
        .map_err(|e| CoralError::Protocol {
            label: "signing",
            detail: format!("{program} failed: {e}"),
        })?;

    if out.status.success() {
        return Ok(out.stdout);
    }
    Err(CoralError::Refused {
        label: "signing",
        detail: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
    })
}

/// Lists the keys available for a format.
///
/// # Errors
/// [`CoralError::Refused`] when the program is missing or refuses.
pub async fn list_keys(
    format: SigningFormat,
    program: &str,
) -> Result<Vec<SigningKey>, CoralError> {
    let program = if program.is_empty() {
        format.default_program()
    } else {
        program
    };

    match format {
        SigningFormat::Ssh => {
            let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
            Ok(home
                .map(|h| ssh_keys_in(&h.join(".ssh")))
                .unwrap_or_default())
        }
        SigningFormat::OpenPgp | SigningFormat::X509 => {
            let out = run(program, &["--list-secret-keys", "--with-colons"], None).await?;
            Ok(parse_secret_keys(&out))
        }
    }
}

/// Creates a signing key and returns it.
///
/// 4096-bit RSA expiring in two years, which is what the reference offers. The passphrase may
/// be empty, meaning an unprotected key; that is the user's decision to make and gpg's own
/// default when it is not asked.
///
/// # Errors
/// [`CoralError::Refused`] with gpg's own message, which is worth showing: it explains a
/// missing agent or a rejected passphrase far better than anything invented here.
pub async fn generate_key(
    program: &str,
    name: &str,
    email: &str,
    passphrase: &SecretString,
) -> Result<SigningKey, CoralError> {
    if name.trim().is_empty() || email.trim().is_empty() {
        return Err(CoralError::Refused {
            label: "signing",
            detail: "a key needs a name and an email address; set user.name and user.email"
                .to_owned(),
        });
    }
    let program = if program.is_empty() { "gpg" } else { program };
    let uid = format!("{} <{}>", name.trim(), email.trim());

    // `loopback` tells gpg to take the passphrase from us rather than open a pinentry window
    // there is nobody to answer, and `--passphrase-fd 0` keeps it off the command line.
    let args = [
        "--batch",
        "--pinentry-mode",
        "loopback",
        "--passphrase-fd",
        "0",
        "--quick-generate-key",
        &uid,
        "rsa4096",
        "sign",
        "2y",
    ];
    run(program, &args, Some(passphrase)).await?;

    let keys = list_keys(SigningFormat::OpenPgp, program).await?;
    keys.into_iter()
        .find(|k| k.label == uid)
        .ok_or_else(|| CoralError::Protocol {
            label: "signing",
            detail: "the key was created but could not be found afterwards".to_owned(),
        })
}
