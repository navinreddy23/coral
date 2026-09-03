//! Coral as a git credential helper.
//!
//! Git speaks a small line protocol on stdin and stdout. Acting as the helper means git never
//! prompts on a terminal the user cannot see, and tokens never appear in a remote URL, a
//! config file, or a process argument list.

use std::collections::BTreeMap;

use secrecy::{ExposeSecret as _, SecretString};

use crate::error::CoralError;

/// The environment variable carrying the running application's nonce.
///
/// Any process on the machine can execute the coral binary. Without a shared secret, another
/// program could invoke `coral credential-helper get` and be handed the user's tokens. The
/// application generates a nonce at startup, passes it to the git children it spawns, and the
/// helper answers only when the value matches.
pub const SESSION_VAR: &str = "CORAL_CREDENTIAL_SESSION";

/// One credential request or response.
///
/// Field order is not significant to git, so a sorted map keeps output deterministic and
/// therefore testable.
///
/// Deliberately not `PartialEq`: `SecretString` does not implement it, because comparing
/// secrets with `==` is how timing attacks get in. `Debug` is safe — the secret redacts itself.
#[derive(Clone, Debug, Default)]
pub struct Credential {
    fields: BTreeMap<String, String>,
    password: Option<SecretString>,
}

impl Credential {
    /// Parses the `key=value` lines git writes, ending at a blank line or end of input.
    ///
    /// # Errors
    /// [`CoralError::Protocol`] on a line that is not `key=value`.
    pub fn parse(input: &str) -> Result<Self, CoralError> {
        let mut c = Self::default();
        for line in input.lines() {
            if line.is_empty() {
                break;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(CoralError::Protocol {
                    label: "credential",
                    detail: "expected key=value".to_owned(),
                });
            };
            if key == "password" {
                c.password = Some(SecretString::from(value.to_owned()));
            } else {
                c.fields.insert(key.to_owned(), value.to_owned());
            }
        }
        Ok(c)
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }

    #[must_use]
    pub fn protocol(&self) -> Option<&str> {
        self.get("protocol")
    }

    #[must_use]
    pub fn host(&self) -> Option<&str> {
        self.get("host")
    }

    #[must_use]
    pub fn username(&self) -> Option<&str> {
        self.get("username")
    }

    #[must_use]
    pub fn password(&self) -> Option<&SecretString> {
        self.password.as_ref()
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.fields.insert(key.to_owned(), value.to_owned());
    }

    pub fn set_password(&mut self, password: SecretString) {
        self.password = Some(password);
    }

    /// The key a credential is stored under: protocol, host and path identify the account.
    ///
    /// Git only sends `path` when `credential.useHttpPath` is set, so its absence is normal
    /// and must not change the key for a host that never sends it.
    #[must_use]
    pub fn storage_key(&self) -> String {
        let protocol = self.protocol().unwrap_or("https");
        let host = self.host().unwrap_or_default();
        match self.get("path") {
            Some(path) => format!("{protocol}://{host}/{path}"),
            None => format!("{protocol}://{host}"),
        }
    }

    /// Renders the response git expects.
    ///
    /// The password is written here and nowhere else: it is deliberately absent from `Debug`,
    /// so it cannot reach a log through an accidental format.
    #[must_use]
    pub fn to_response(&self) -> String {
        let mut out = String::new();
        for (k, v) in &self.fields {
            out.push_str(k);
            out.push('=');
            out.push_str(v);
            out.push('\n');
        }
        if let Some(p) = &self.password {
            out.push_str("password=");
            out.push_str(p.expose_secret());
            out.push('\n');
        }
        out
    }
}

/// What git asked the helper to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Supply a credential, or say nothing.
    Get,
    /// Remember one that worked.
    Store,
    /// Forget one that did not.
    Erase,
}

impl Action {
    /// # Errors
    /// [`CoralError::Refused`] for anything git does not define.
    pub fn parse(word: &str) -> Result<Self, CoralError> {
        match word {
            "get" => Ok(Self::Get),
            "store" => Ok(Self::Store),
            "erase" => Ok(Self::Erase),
            other => Err(CoralError::Refused {
                label: "credential-helper",
                detail: format!("unknown action {other:?}"),
            }),
        }
    }
}

/// Whether this invocation is allowed to touch stored credentials.
///
/// Compared in full rather than by prefix, and a missing or empty expectation denies rather
/// than allows: a helper that answers when unconfigured is worse than one that never answers.
#[must_use]
pub fn session_matches(expected: Option<&str>, provided: Option<&str>) -> bool {
    match (expected, provided) {
        (Some(e), Some(p)) if !e.is_empty() => e == p,
        _ => false,
    }
}

/// Where credentials are kept.
pub trait CredentialStore {
    /// # Errors
    /// Implementation-defined storage failures.
    fn get(&self, key: &str, username: &str) -> Result<Option<SecretString>, CoralError>;

    /// # Errors
    /// Implementation-defined storage failures.
    fn set(&self, key: &str, username: &str, secret: &SecretString) -> Result<(), CoralError>;

    /// # Errors
    /// Implementation-defined storage failures.
    fn delete(&self, key: &str, username: &str) -> Result<(), CoralError>;
}

/// The OS keyring: Keychain on macOS, Credential Manager on Windows, Secret Service on Linux.
pub struct Keyring;

impl Keyring {
    fn entry(key: &str, username: &str) -> Result<keyring::Entry, CoralError> {
        keyring::Entry::new(&format!("coral:{key}"), username).map_err(|e| CoralError::Protocol {
            label: "keyring",
            detail: e.to_string(),
        })
    }
}

impl CredentialStore for Keyring {
    fn get(&self, key: &str, username: &str) -> Result<Option<SecretString>, CoralError> {
        match Self::entry(key, username)?.get_password() {
            Ok(p) => Ok(Some(SecretString::from(p))),
            // A missing entry is the normal case, not a failure.
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(CoralError::Protocol {
                label: "keyring",
                detail: e.to_string(),
            }),
        }
    }

    fn set(&self, key: &str, username: &str, secret: &SecretString) -> Result<(), CoralError> {
        Self::entry(key, username)?
            .set_password(secret.expose_secret())
            .map_err(|e| CoralError::Protocol {
                label: "keyring",
                detail: e.to_string(),
            })
    }

    fn delete(&self, key: &str, username: &str) -> Result<(), CoralError> {
        match Self::entry(key, username)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(CoralError::Protocol {
                label: "keyring",
                detail: e.to_string(),
            }),
        }
    }
}

/// Runs one credential-helper invocation.
///
/// Returns what should be written to stdout. An unauthorised or unknown request produces an
/// empty response, which git reads as "this helper has nothing" and falls through to its
/// normal handling — never an error, because a failing helper aborts the whole operation.
///
/// # Errors
/// [`CoralError::Protocol`] if the request does not parse.
pub fn respond(
    store: &dyn CredentialStore,
    action: Action,
    request: &str,
    authorised: bool,
) -> Result<String, CoralError> {
    let mut credential = Credential::parse(request)?;
    if !authorised {
        return Ok(String::new());
    }
    let key = credential.storage_key();

    match action {
        Action::Get => {
            // A token push sends only the protocol and host: the remote URL carries no
            // username, so git has none to offer. Answering nothing there sends git off to
            // ask for one on a terminal it has been told it cannot use, and the push fails
            // with a prompt nobody sees. The name last stored for the host stands in.
            let username = match credential.username().map(str::to_owned) {
                Some(name) => name,
                None => match store.get(&remembered_user_key(&key), REMEMBERED_USER)? {
                    Some(name) => {
                        let name = name.expose_secret().to_owned();
                        credential.set("username", &name);
                        name
                    }
                    None => return Ok(String::new()),
                },
            };
            match store.get(&key, &username)? {
                Some(secret) => {
                    credential.set_password(secret);
                    Ok(credential.to_response())
                }
                None => Ok(String::new()),
            }
        }
        Action::Store => {
            if let (Some(username), Some(password)) = (
                credential.username().map(str::to_owned),
                credential.password(),
            ) {
                store.set(&key, &username, password)?;
                // So a later request that carries no username can still be answered.
                store.set(
                    &remembered_user_key(&key),
                    REMEMBERED_USER,
                    &SecretString::from(username),
                )?;
            }
            Ok(String::new())
        }
        Action::Erase => {
            // Forget the remembered name too, or the next request answers with a user whose
            // password has just been deleted and git reports a wrong password rather than a
            // missing one.
            store.delete(&remembered_user_key(&key), REMEMBERED_USER)?;
            if let Some(username) = credential.username() {
                store.delete(&key, username)?;
            }
            Ok(String::new())
        }
    }
}

/// The account name the last-used username is filed under.
const REMEMBERED_USER: &str = "username";

/// Where that name lives.
///
/// A separate storage key rather than a reserved account under the real one: a git username
/// can be almost any string, and there is no account name that is safely not one. U+0001 is
/// not a character a URL can contain, so this key cannot collide with a host's own.
fn remembered_user_key(key: &str) -> String {
    format!("{key}\u{1}user")
}

/// The arguments that make git use coral as its only credential helper.
///
/// The empty value first clears any helper the user's config already set, so ours is
/// authoritative for the hosts it knows. For hosts it does not know it answers nothing, and
/// git falls through to its normal behaviour.
///
/// The nonce goes on the command line and is compared against the one in the environment,
/// which git children inherit and unrelated processes do not. git appends the action after
/// whatever is configured here, so the invocation reads `… --session <nonce> get`.
///
/// The `!` prefix is required and means what it says here: `credential.helper` treats a value
/// starting with `!` as a shell command rather than a `git-credential-` suffix to look up.
#[must_use]
pub fn helper_args(coral_binary: &std::path::Path, session: &str) -> Vec<String> {
    vec![
        "-c".to_owned(),
        "credential.helper=".to_owned(),
        "-c".to_owned(),
        format!(
            "credential.helper=!'{}' credential-helper --session '{session}'",
            coral_binary.display()
        ),
    ]
}

/// How the running front end presents itself to git as a credential helper.
///
/// Set once at startup. Until it is, network commands carry no helper and git behaves as it
/// normally would, which is what the CLI wants when it is driven by a person at a terminal
/// who has their own helper configured.
static HELPER: std::sync::OnceLock<Helper> = std::sync::OnceLock::new();

struct Helper {
    binary: std::path::PathBuf,
    session: String,
}

/// Makes Coral git's credential helper for every network command from now on.
///
/// `binary` must be a program that answers `credential-helper`. Both front ends do: the CLI as
/// a subcommand, the application by handling it before it starts a window.
///
/// The nonce is generated by the caller and passed to git children in the environment. Any
/// process on the machine can execute the binary, so without it another program could invoke
/// the helper and be handed the user's tokens.
pub fn configure(binary: std::path::PathBuf, session: String) {
    let _ = HELPER.set(Helper { binary, session });
}

/// A fresh nonce for [`configure`].
///
/// From the OS random source. Falling back to anything derived from the clock or the pid would
/// be guessable by exactly the local process this is meant to keep out, so a failure to read
/// randomness disables the helper rather than weakening it.
#[must_use]
pub fn new_session() -> Option<String> {
    let mut bytes = [0_u8; 24];
    getrandom::fill(&mut bytes).ok()?;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{b:02x}");
    }
    Some(out)
}

/// The `-c` arguments and environment a network command needs to reach the helper.
#[must_use]
pub(crate) fn helper_config() -> Option<(Vec<String>, String)> {
    let helper = HELPER.get()?;
    Some((
        helper_args(&helper.binary, &helper.session),
        helper.session.clone(),
    ))
}
