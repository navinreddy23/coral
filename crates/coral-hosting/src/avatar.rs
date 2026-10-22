//! An author's picture: where it can be found for an email address, and fetching it.
//!
//! Two sources, tried in order. A GitHub `noreply` address names its account, so the account's
//! own picture is asked for and nothing about the address leaves the machine but the account it
//! already names. Anything else is asked of Gravatar by the SHA-256 of the address, which is how
//! Gravatar is addressed and the only thing it is sent.
//! <https://docs.gravatar.com/api/avatars/images/>

use std::fmt::Write as _;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::provider::HostingError;

/// Pixels on a side asked for. A node is up to about 30 pixels across and a high-density screen
/// doubles that; a 64-pixel picture was being enlarged there, and looked it.
pub const SIZE: u32 = 128;
/// A picture this size is not a picture anybody uploaded as an avatar.
const LIMIT: usize = 512 * 1024;

/// Where to look for the picture of whoever writes as `email`, best first.
#[must_use]
pub fn sources(email: &str) -> Vec<String> {
    let email = email.trim().to_ascii_lowercase();
    let mut out = Vec::new();
    if let Some(github) = github_noreply(&email) {
        out.push(github);
    }
    if email.contains('@') {
        out.push(gravatar(&email));
    }
    out
}

/// `12345+login@users.noreply.github.com` names account 12345, and the older
/// `login@users.noreply.github.com` names the login alone.
/// <https://docs.github.com/en/account-and-profile/setting-up-and-managing-your-personal-account-on-github/managing-email-preferences/setting-your-commit-email-address>
fn github_noreply(email: &str) -> Option<String> {
    let local = email.strip_suffix("@users.noreply.github.com")?;
    match local.split_once('+') {
        Some((id, login)) if is_number(id) && is_login(login) => Some(format!(
            "https://avatars.githubusercontent.com/u/{id}?s={SIZE}"
        )),
        None if is_login(local) => Some(format!("https://github.com/{local}.png?size={SIZE}")),
        _ => None,
    }
}

fn is_number(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// GitHub's own rule for a login: letters, digits and single hyphens, which is also what keeps
/// anything else out of the URL.
fn is_login(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 39
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !s.starts_with('-')
}

/// The address trimmed, lowercased and hashed: Gravatar's key for it, and a name to keep its
/// picture under that does not spell the address out.
#[must_use]
pub fn key(email: &str) -> String {
    let hash = Sha256::digest(email.trim().to_ascii_lowercase().as_bytes());
    hash.iter().fold(String::with_capacity(64), |mut hex, b| {
        let _ = write!(hex, "{b:02x}");
        hex
    })
}

/// `d=404` makes an address with no picture an answer of its own rather than a generated one.
fn gravatar(email: &str) -> String {
    format!("https://gravatar.com/avatar/{}?s={SIZE}&d=404", key(email))
}

/// Fetches pictures, with nothing of the user's attached: no token, no cookie.
pub struct Fetcher {
    http: reqwest::Client,
}

impl Fetcher {
    /// # Errors
    /// [`HostingError::Transport`] if the HTTP stack cannot be built.
    pub fn new() -> Result<Self, HostingError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent(concat!("coral/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| HostingError::Transport(e.to_string()))?;
        Ok(Self { http })
    }

    /// The first picture any source has, or `None` when every source says there is none.
    ///
    /// # Errors
    /// [`HostingError::Transport`] when a source could not be asked, which says nothing about
    /// whether the picture exists and so must not be remembered as its absence.
    pub async fn fetch(&self, email: &str) -> Result<Option<Vec<u8>>, HostingError> {
        for url in sources(email) {
            if let Some(bytes) = self.one(&url).await? {
                return Ok(Some(bytes));
            }
        }
        Ok(None)
    }

    async fn one(&self, url: &str) -> Result<Option<Vec<u8>>, HostingError> {
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| HostingError::Transport(e.to_string()))?;
        if !response.status().is_success() {
            return Ok(None);
        }
        let is_image = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|t| t.starts_with("image/"));
        let bytes = response
            .bytes()
            .await
            .map_err(|e| HostingError::Transport(e.to_string()))?;
        Ok((is_image && bytes.len() <= LIMIT).then(|| bytes.to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use super::sources;

    #[test]
    fn a_github_noreply_address_asks_github_for_that_account() {
        assert_eq!(
            sources("12345+octocat@users.noreply.github.com")[0],
            "https://avatars.githubusercontent.com/u/12345?s=128"
        );
        assert_eq!(
            sources("octocat@users.noreply.github.com")[0],
            "https://github.com/octocat.png?size=128"
        );
    }

    #[test]
    fn anything_else_asks_gravatar_by_the_hash_of_the_address() {
        // The address trimmed and lowercased, then hashed; the hash computed apart from this code,
        // with `sha256sum`.
        assert_eq!(
            sources("  MyEmailAddress@example.com "),
            [
                "https://gravatar.com/avatar/84059b07d4be67b806386c0aad8070a23f18836bbaae342275dc0a83414c32ee?s=128&d=404"
            ]
        );
    }

    #[test]
    fn a_noreply_address_falls_back_to_gravatar_too() {
        assert_eq!(sources("1+a@users.noreply.github.com").len(), 2);
    }

    #[test]
    fn a_login_that_could_reach_another_url_is_not_used() {
        for email in [
            "a/b@users.noreply.github.com",
            "1+a?x=1@users.noreply.github.com",
            "-a@users.noreply.github.com",
            "x+a@users.noreply.github.com",
        ] {
            let got = sources(email);
            assert!(
                got.iter().all(|u| u.starts_with("https://gravatar.com/")),
                "{email}: {got:?}"
            );
        }
    }

    #[test]
    fn something_that_is_not_an_address_asks_nobody() {
        assert_eq!(sources(""), Vec::<String>::new());
        assert_eq!(sources("not an address"), Vec::<String>::new());
    }
}
