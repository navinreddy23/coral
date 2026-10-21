//! Whether a newer Coral has been published, and installing it where that can be done.
//!
//! Asked from About, or at launch only when the user has turned that on: the window contacts
//! nothing on its own by default. Both go through Tauri's updater, which reads the newest
//! release's `latest.json`, refuses any download whose signature does not verify against the
//! public key in `tauri.conf.json`, and installs over the running copy. The release names one
//! download per kind of install, so an `AppImage` is replaced by an `AppImage`, a `.deb` is
//! installed with `dpkg` through a graphical sudo prompt, and the macOS app and the Windows
//! installer update themselves. A build that was not installed from a bundle has no entry, and
//! is offered the release page.

use std::sync::{Arc, Mutex, PoisonError};

use tauri::Emitter;
use tauri_plugin_updater::UpdaterExt;

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
    /// True when the updater can install `latest` over this copy in place.
    pub installable: bool,
}

impl UpdateCheck {
    #[must_use]
    pub fn found(current: &str, latest: &str, installable: bool) -> Self {
        let newer = is_newer(current, latest);
        Self {
            current: current.to_owned(),
            latest: latest.to_owned(),
            newer,
            url: format!("https://github.com/navinreddy23/coral/releases/tag/v{latest}"),
            installable: newer && installable,
        }
    }
}

/// How far a download has got, sent while an update installs.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub received: u64,
    /// `None` when the server did not say how large the download is.
    pub total: Option<u64>,
}

/// Whether `candidate` is a later version than `current`.
///
/// Compared as numbers, so 1.10.0 follows 1.9.0, and a pre-release comes before the release it
/// precedes. Anything that is not `major.minor.patch` is never newer: a check that offered a
/// version it could not read would be offering something nobody can vouch for.
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

/// Reads the newest release's `latest.json` and compares it with this build.
///
/// # Errors
/// Refused when the release cannot be reached or read.
#[tauri::command]
pub async fn check_for_update(app: tauri::AppHandle) -> Result<UpdateCheck, IpcError> {
    let current = app.package_info().version.to_string();
    // The updater says only whether there is something to install. The version it was offered
    // is kept here, so a copy with nothing to install can still say what the newest release is.
    let offered = Arc::new(Mutex::new(None::<String>));
    let seen = Arc::clone(&offered);
    let updater = app
        .updater_builder()
        .version_comparator(move |have, release| {
            *seen.lock().unwrap_or_else(PoisonError::into_inner) =
                Some(release.version.to_string());
            release.version > have
        })
        .build()
        .map_err(|e| refused("check for updates", &e.to_string()))?;
    let found = updater.check().await;
    let latest = offered
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    match (found, latest) {
        (Ok(Some(update)), _) => Ok(UpdateCheck::found(&current, &update.version, true)),
        // No entry for this kind of install: a build run from the source tree, say. The release
        // is still newer or not, and the page still has its downloads.
        (Ok(None) | Err(tauri_plugin_updater::Error::TargetsNotFound(_)), Some(latest)) => {
            Ok(UpdateCheck::found(&current, &latest, false))
        }
        (Err(e), _) => Err(refused("check for updates", &e.to_string()).into()),
        (Ok(None), None) => {
            Err(refused("check for updates", "the release did not name a version").into())
        }
    }
}

/// Downloads the newest release, verifies its signature, installs it over this copy, and
/// restarts into it. Progress goes to the window as `coral://update-progress`.
///
/// # Errors
/// Refused when there is nothing to install, or the download, the signature or the install
/// fails. The copy already installed is left as it was.
#[tauri::command]
pub async fn install_update(app: tauri::AppHandle) -> Result<(), IpcError> {
    let updater = app
        .updater()
        .map_err(|e| refused("install the update", &e.to_string()))?;
    let update = updater
        .check()
        .await
        .map_err(|e| refused("install the update", &e.to_string()))?
        .ok_or_else(|| {
            refused(
                "install the update",
                "there is no newer release for this install",
            )
        })?;
    let mut received = 0u64;
    update
        .download_and_install(
            |chunk, total| {
                received += chunk as u64;
                let _ = app.emit(
                    "coral://update-progress",
                    UpdateProgress { received, total },
                );
            },
            || {},
        )
        .await
        .map_err(|e| refused("install the update", &e.to_string()))?;
    app.restart()
}

fn refused(label: &'static str, detail: &str) -> coral_core::CoralError {
    coral_core::CoralError::Refused {
        label,
        detail: detail.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_later_release_is_offered_with_its_page() {
        let check = UpdateCheck::found("1.2.3", "1.2.4", true);
        assert!(check.newer && check.installable);
        assert_eq!(
            check.url,
            "https://github.com/navinreddy23/coral/releases/tag/v1.2.4"
        );
    }

    #[test]
    fn a_later_release_with_no_entry_for_this_install_is_offered_as_a_page() {
        let check = UpdateCheck::found("1.2.3", "1.2.4", false);
        assert!(check.newer);
        assert!(!check.installable);
    }

    #[test]
    fn this_build_being_the_latest_offers_nothing() {
        let check = UpdateCheck::found("1.2.3", "1.2.3", true);
        assert!(!check.newer);
        assert!(
            !check.installable,
            "nothing to install is never installable"
        );
    }

    #[test]
    fn a_build_ahead_of_the_latest_release_offers_nothing() {
        // A build from master after a release is newer than anything published.
        assert!(!UpdateCheck::found("1.3.0", "1.2.4", true).newer);
    }

    #[test]
    fn the_answer_is_shaped_for_the_window() {
        let json = serde_json::to_value(UpdateCheck::found("1.2.3", "1.2.4", true)).unwrap();
        assert_eq!(json["current"], "1.2.3");
        assert_eq!(json["latest"], "1.2.4");
        assert_eq!(json["newer"], true);
        assert_eq!(json["installable"], true);
        assert!(json["url"].is_string());
    }

    #[test]
    fn compares_versions_by_number_not_by_text() {
        assert!(is_newer("1.2.3", "1.2.4"));
        assert!(is_newer("1.2.3", "1.3.0"));
        assert!(is_newer("1.2.3", "2.0.0"));
        // As text, "1.10.0" sorts before "1.9.0".
        assert!(is_newer("1.9.0", "1.10.0"));
        assert!(!is_newer("1.10.0", "1.9.0"));
    }

    #[test]
    fn the_same_version_or_an_older_one_is_not_newer() {
        assert!(!is_newer("1.2.3", "1.2.3"));
        assert!(!is_newer("1.2.3", "1.2.2"));
        assert!(!is_newer("1.2.3", "0.9.9"));
    }

    #[test]
    fn a_pre_release_is_older_than_its_release() {
        assert!(!is_newer("1.3.0", "1.3.0-rc.1"));
        assert!(is_newer("1.2.3", "1.3.0-rc.1"));
        assert!(is_newer("1.3.0-rc.1", "1.3.0"));
    }

    #[test]
    fn a_version_it_cannot_read_is_never_offered_as_an_update() {
        assert!(!is_newer("1.2.3", "nightly"));
        assert!(!is_newer("1.2.3", ""));
        assert!(!is_newer("1.2.3", "1.2"));
    }
}
