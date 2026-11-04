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

/// Which of several logins on one host a token belongs to.
///
/// One person can have a work account and a personal one on github.com, and the origin is the
/// same for both, so the service name cannot tell them apart. The keyring's account field can,
/// and it was carrying a constant.
///
/// Not the user's login name: that is not known until the token has been used once, and an
/// entry you cannot find until after a successful request is no place to keep the thing the
/// request needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account(String);

impl Account {
    /// The account every profile falls back to, and the only one the CLI knows.
    ///
    /// Its name is the constant that was stored before profiles existed, so upgrading signs
    /// nobody out.
    #[must_use]
    pub fn shared() -> Self {
        Self("api-token".to_owned())
    }

    /// The account belonging to one profile alone.
    #[must_use]
    pub fn of_profile(id: &str) -> Self {
        Self(format!("profile:{id}"))
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.0
    }
}

/// How a token is presented to the provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Auth {
    /// A header of the host's own choosing, and its value.
    Header(&'static str, String),
    /// HTTP Basic, which the HTTP client encodes.
    Basic { user: String, password: String },
}

/// How each provider wants its token.
///
/// GitHub takes a bearer token like any OAuth API. GitLab has its own header and refuses a
/// bearer personal access token, which is the confusing half of getting this wrong: the
/// request is well formed and comes back 401. Gitea wants the word `token`. Bitbucket takes a
/// repository or workspace access token as a bearer, and an API token as Basic with the
/// account it belongs to, which is why that one is stored as `user:secret`. Azure DevOps
/// takes a personal access token as the password of Basic with no user at all.
#[must_use]
pub fn auth(kind: HostKind, token: &SecretString) -> Auth {
    use secrecy::ExposeSecret as _;
    let secret = token.expose_secret();
    match kind {
        HostKind::GitHub => Auth::Header("Authorization", format!("Bearer {secret}")),
        HostKind::GitLab => Auth::Header("PRIVATE-TOKEN", secret.to_owned()),
        HostKind::Gitea => Auth::Header("Authorization", format!("token {secret}")),
        HostKind::Bitbucket => match secret.split_once(':') {
            Some((user, password)) => Auth::Basic {
                user: user.to_owned(),
                password: password.to_owned(),
            },
            None => Auth::Header("Authorization", format!("Bearer {secret}")),
        },
        HostKind::Azure => Auth::Basic {
            user: String::new(),
            password: secret.to_owned(),
        },
    }
}

/// Reads the token stored for a host.
///
/// # Errors
/// [`HostingError::Transport`] when the keyring itself is unreachable — a headless session
/// with no secret service, most often — which is distinct from there being no token.
pub fn load(host: &Host, account: &Account) -> Result<Option<SecretString>, crate::HostingError> {
    match entry(host, account)?.get_password() {
        Ok(secret) => Ok(Some(SecretString::from(secret))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(crate::HostingError::Transport(e.to_string())),
    }
}

/// Stores a token for a host.
///
/// # Errors
/// [`HostingError::Transport`] if the keyring refuses the write.
pub fn store(
    host: &Host,
    account: &Account,
    token: &SecretString,
) -> Result<(), crate::HostingError> {
    use secrecy::ExposeSecret as _;
    entry(host, account)?
        .set_password(token.expose_secret())
        .map_err(|e| crate::HostingError::Transport(e.to_string()))
}

/// Forgets the token for a host. Succeeds when there was none.
///
/// # Errors
/// [`HostingError::Transport`] if the keyring refuses the delete.
pub fn forget(host: &Host, account: &Account) -> Result<(), crate::HostingError> {
    match entry(host, account)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(crate::HostingError::Transport(e.to_string())),
    }
}

fn entry(host: &Host, account: &Account) -> Result<keyring::Entry, crate::HostingError> {
    keyring::Entry::new(&keyring_service(host), account.name())
        .map_err(|e| crate::HostingError::Transport(e.to_string()))
}
