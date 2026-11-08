//! How often the window fetches one repository on its own, when that repository says.
//!
//! The window has an interval of its own for every repository; this is the exception a single
//! repository can make to it — off for one whose remote is slow or metered, more often for one
//! a team pushes to all day. Kept in the repository's config under Coral's own section, so it
//! goes where the repository goes and nothing else has to store it.

use crate::config::{AppConfig, Scope};
use crate::error::CoralError;
use crate::process::GitRunner;
use crate::repo::RepoLocation;

const KEY: &str = "coral.autoFetch";

// A day. Longer than that is not an interval anybody means, and a typo of a large number
// would otherwise read as "never" without saying so.
const LONGEST: u16 = 24 * 60;

/// What this repository asks for instead of the window's own interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AutoFetch {
    Off,
    Every { minutes: u16 },
}

impl AutoFetch {
    // Anything unreadable is no setting at all, so the window's own interval applies: a
    // hand-edited config should not stop fetching without a word.
    fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.eq_ignore_ascii_case("off") {
            return Some(Self::Off);
        }
        match value.parse::<u16>() {
            Ok(minutes @ 1..=LONGEST) => Some(Self::Every { minutes }),
            _ => None,
        }
    }

    fn written(self) -> String {
        match self {
            Self::Off => "off".to_owned(),
            Self::Every { minutes } => minutes.to_string(),
        }
    }
}

impl RepoLocation {
    /// The interval this repository asks for, or `None` when it leaves it to the window.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn auto_fetch(&self, runner: &GitRunner) -> Result<Option<AutoFetch>, CoralError> {
        Ok(self
            .config_get(runner, Scope::Effective(AppConfig(None)), KEY)
            .await?
            .as_deref()
            .and_then(AutoFetch::parse))
    }

    /// Sets this repository's interval, or hands it back to the window with `None`.
    ///
    /// # Errors
    /// [`CoralError::Refused`] for no minutes or more than a day; otherwise git failures.
    pub async fn set_auto_fetch(
        &self,
        runner: &GitRunner,
        to: Option<AutoFetch>,
    ) -> Result<(), CoralError> {
        if let Some(AutoFetch::Every { minutes }) = to
            && !(1..=LONGEST).contains(&minutes)
        {
            return Err(CoralError::Refused {
                label: "fetch automatically",
                detail: format!("the interval has to be between 1 and {LONGEST} minutes"),
            });
        }
        let value = to.map(AutoFetch::written);
        self.config_put(runner, Scope::Repository, KEY, value.as_deref())
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::AutoFetch;

    #[test]
    fn reads_off_and_minutes_and_nothing_else() {
        assert_eq!(AutoFetch::parse(" OFF "), Some(AutoFetch::Off));
        assert_eq!(AutoFetch::parse("5"), Some(AutoFetch::Every { minutes: 5 }));
        assert_eq!(AutoFetch::parse("0"), None);
        assert_eq!(AutoFetch::parse("1441"), None);
        assert_eq!(AutoFetch::parse("ten"), None);
    }
}
