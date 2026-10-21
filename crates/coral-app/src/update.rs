//! Whether a newer Coral has been published, asked from About and nowhere else.
//!
//! Nothing installs itself: the answer names the version and the page its downloads are on.
//! Updating in place needs builds signed for it, and checking on its own would make the window
//! contact GitHub without being asked.

use coral_hosting::release::{self, Release};

use crate::commands::IpcError;

/// What the check found.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    /// The version this build is.
    pub current: String,
    /// The newest published version.
    pub latest: String,
    /// True when `latest` is later than `current`.
    pub newer: bool,
    /// The release page of `latest`.
    pub url: String,
}

impl UpdateCheck {
    #[must_use]
    pub fn against(current: &str, latest: Release) -> Self {
        Self {
            current: current.to_owned(),
            newer: release::is_newer(current, &latest.version),
            latest: latest.version,
            url: latest.url,
        }
    }
}

/// Asks GitHub for the newest published release and compares it with this build.
///
/// # Errors
/// Refused when GitHub cannot be reached, refuses, or answers with something that is not a
/// release.
#[tauri::command]
pub async fn check_for_update() -> Result<UpdateCheck, IpcError> {
    let latest = release::latest()
        .await
        .map_err(|e| coral_core::CoralError::Refused {
            label: "check for updates",
            detail: e.to_string(),
        })?;
    Ok(UpdateCheck::against(env!("CARGO_PKG_VERSION"), latest))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn published(version: &str) -> Release {
        Release {
            version: version.to_owned(),
            url: format!("https://github.com/navinreddy23/coral/releases/tag/v{version}"),
        }
    }

    #[test]
    fn a_later_release_is_offered_with_its_page() {
        let check = UpdateCheck::against("1.2.3", published("1.2.4"));
        assert!(check.newer);
        assert_eq!(check.latest, "1.2.4");
        assert!(check.url.ends_with("/v1.2.4"));
    }

    #[test]
    fn this_build_being_the_latest_offers_nothing() {
        assert!(!UpdateCheck::against("1.2.3", published("1.2.3")).newer);
    }

    #[test]
    fn a_build_ahead_of_the_latest_release_offers_nothing() {
        // A build from master after a release is newer than anything published.
        assert!(!UpdateCheck::against("1.3.0", published("1.2.4")).newer);
    }

    #[test]
    fn the_answer_is_shaped_for_the_window() {
        let json = serde_json::to_value(UpdateCheck::against("1.2.3", published("1.2.4"))).unwrap();
        assert_eq!(json["current"], "1.2.3");
        assert_eq!(json["latest"], "1.2.4");
        assert_eq!(json["newer"], true);
        assert!(json["url"].is_string());
    }
}
