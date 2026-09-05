//! What this build of Coral is, so a bug report can name it.

/// The build the window is running.
///
/// Three facts rather than one string, because a report that says only "0.1.0" narrows nothing
/// down: hundreds of builds carry a version between two releases, and a debug build is slow in
/// ways that are worth knowing before anyone investigates a complaint about speed.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    /// The crate version, which is the workspace version and the one the bundles carry.
    pub number: &'static str,
    /// The commit it was built from. `None` when there was none to name: a source tarball, or
    /// a machine without git. Absent is the honest answer; a made-up one is not.
    pub commit: Option<&'static str>,
    /// True for a debug build. Worth saying, since it is several times slower than a release.
    pub debug: bool,
}

impl Version {
    #[must_use]
    pub const fn current() -> Self {
        Self {
            number: env!("CARGO_PKG_VERSION"),
            commit: option_env!("CORAL_COMMIT"),
            debug: cfg!(debug_assertions),
        }
    }
}

/// What this build of Coral is.
#[tauri::command]
#[must_use]
pub const fn app_version() -> Version {
    Version::current()
}

#[cfg(test)]
mod tests {
    use super::Version;

    /// The version the window shows has to be the one the bundles carry, and both come from the
    /// crate. `tauri.conf.json` used to repeat it, where it could drift a release behind without
    /// anything failing; it no longer states one, and Tauri takes the crate's.
    #[test]
    fn the_version_is_the_crate_version_and_is_a_semver_triple() {
        let v = Version::current();
        assert_eq!(v.number, env!("CARGO_PKG_VERSION"));
        let parts: Vec<&str> = v.number.split('.').collect();
        assert_eq!(
            parts.len(),
            3,
            "expected major.minor.patch, got {}",
            v.number
        );
        assert!(
            parts
                .iter()
                .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())),
            "every part has to be a number: {}",
            v.number
        );
    }

    /// The config must not state a version of its own, or the two can disagree.
    #[test]
    fn the_tauri_config_does_not_carry_a_second_version() {
        let config = include_str!("../tauri.conf.json");
        let parsed: serde_json::Value = serde_json::from_str(config).expect("config is JSON");
        assert!(
            parsed.get("version").is_none(),
            "tauri.conf.json states a version; it should inherit the crate's"
        );
    }

    #[test]
    fn a_debug_build_says_so() {
        assert_eq!(Version::current().debug, cfg!(debug_assertions));
    }
}
