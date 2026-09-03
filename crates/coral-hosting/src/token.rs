use secrecy::SecretString;

use crate::provider::{Host, HostKind};

/// Where a host's token lives in the OS keyring.
///
/// Keyed by origin rather than by kind: someone can be signed in to github.com and to a
/// company instance at once, and the two tokens are not interchangeable.
#[must_use]
pub fn keyring_service(host: &Host) -> String {
    format!("dev.coral.app:{}", host.origin)
}

/// The account name stored alongside the token.
///
/// A constant rather than the user's login: the login is not known until the token has been
/// used once, and a keyring entry that cannot be found until after a successful request is
/// not a place to keep the thing needed to make it.
pub const KEYRING_ACCOUNT: &str = "api-token";

/// How a token is presented to the provider.
///
/// GitHub takes a bearer token like any OAuth API. GitLab has its own header and refuses a
/// bearer personal access token, which is the confusing half of getting this wrong: the
/// request is well formed and comes back 401.
#[must_use]
pub fn auth_header(kind: HostKind, token: &SecretString) -> (&'static str, String) {
    use secrecy::ExposeSecret as _;
    match kind {
        HostKind::GitHub => ("Authorization", format!("Bearer {}", token.expose_secret())),
        HostKind::GitLab => ("PRIVATE-TOKEN", token.expose_secret().to_owned()),
    }
}

/// Reads the token stored for a host.
///
/// # Errors
/// [`HostingError::Transport`] when the keyring itself is unreachable — a headless session
/// with no secret service, most often — which is distinct from there being no token.
pub fn load(host: &Host) -> Result<Option<SecretString>, crate::HostingError> {
    match entry(host)?.get_password() {
        Ok(secret) => Ok(Some(SecretString::from(secret))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(crate::HostingError::Transport(e.to_string())),
    }
}

/// Stores a token for a host.
///
/// # Errors
/// [`HostingError::Transport`] if the keyring refuses the write.
pub fn store(host: &Host, token: &SecretString) -> Result<(), crate::HostingError> {
    use secrecy::ExposeSecret as _;
    entry(host)?
        .set_password(token.expose_secret())
        .map_err(|e| crate::HostingError::Transport(e.to_string()))
}

/// Forgets the token for a host. Succeeds when there was none.
///
/// # Errors
/// [`HostingError::Transport`] if the keyring refuses the delete.
pub fn forget(host: &Host) -> Result<(), crate::HostingError> {
    match entry(host)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(crate::HostingError::Transport(e.to_string())),
    }
}

fn entry(host: &Host) -> Result<keyring::Entry, crate::HostingError> {
    keyring::Entry::new(&keyring_service(host), KEYRING_ACCOUNT)
        .map_err(|e| crate::HostingError::Transport(e.to_string()))
}
