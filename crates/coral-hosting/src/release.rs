//! Coral's own newest release, for the check in About.
//!
//! Only ever asked when someone presses the button. The window contacts nothing on its own,
//! and an update check that ran at startup would be the one exception to that.

use crate::provider::HostingError;

/// GitHub's newest published release. Drafts and pre-releases are not returned by it.
pub const LATEST_URL: &str = "https://api.github.com/repos/navinreddy23/coral/releases/latest";

/// A published release, as much of it as the check needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    /// The version, without the tag's leading `v`.
    pub version: String,
    /// The release page, where the downloads are.
    pub url: String,
}

#[derive(serde::Deserialize)]
struct Latest {
    tag_name: String,
    html_url: String,
}

/// Reads the body of [`LATEST_URL`].
///
/// # Errors
/// [`HostingError::Malformed`] when the body is not a release, which is also what GitHub
/// answers for a repository with nothing published.
pub fn parse_latest(body: &[u8]) -> Result<Release, HostingError> {
    let latest: Latest =
        serde_json::from_slice(body).map_err(|e| HostingError::Malformed(e.to_string()))?;
    Ok(Release {
        version: latest
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&latest.tag_name)
            .to_owned(),
        url: latest.html_url,
    })
}

/// Whether `candidate` is a later version than `current`.
///
/// Compared as numbers, so 1.10.0 follows 1.9.0, and a pre-release comes before the release it
/// precedes. Anything that is not `major.minor.patch` is never newer: a check that offered a
/// tag it could not read would be offering something nobody can vouch for.
#[must_use]
pub fn is_newer(current: &str, candidate: &str) -> bool {
    match (parts(current), parts(candidate)) {
        (Some(have), Some(offered)) => offered > have,
        _ => false,
    }
}

/// `major.minor.patch` as numbers, then whether it is a full release, so that a pre-release
/// sorts below the release of the same number.
fn parts(version: &str) -> Option<(u64, u64, u64, bool)> {
    let (numbers, pre) = match version.split_once('-') {
        Some((numbers, pre)) => (numbers, Some(pre)),
        None => (version, None),
    };
    let mut fields = numbers.split('.').map(str::parse::<u64>);
    let major = fields.next()?.ok()?;
    let minor = fields.next()?.ok()?;
    let patch = fields.next()?.ok()?;
    if fields.next().is_some() {
        return None;
    }
    Some((major, minor, patch, pre.is_none()))
}

/// Asks GitHub for the newest published release.
///
/// # Errors
/// [`HostingError::Transport`] when GitHub cannot be reached, [`HostingError::Api`] when it
/// refuses, and [`HostingError::Malformed`] when the answer is not a release.
pub async fn latest() -> Result<Release, HostingError> {
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent(concat!("coral/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| HostingError::Transport(e.to_string()))?;
    let response = http
        .get(LATEST_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| HostingError::Transport(e.to_string()))?;
    let status = response.status().as_u16();
    let body = response
        .bytes()
        .await
        .map_err(|e| HostingError::Transport(e.to_string()))?;
    if !(200..300).contains(&status) {
        return Err(HostingError::Api {
            status,
            detail: crate::client::describe_failure(status, &body),
        });
    }
    parse_latest(&body)
}
