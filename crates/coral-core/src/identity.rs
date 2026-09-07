//! Who a repository records as the author of its commits.
//!
//! `user.name` and `user.email`, layered the way signing and ssh are: an app-level default in
//! the user's own git configuration, and a per-repository override that beats it. Kept as its
//! own noun rather than folded into signing, because a repository can have an identity without
//! signing anything.

use crate::config::{AppConfig, Scope};
use crate::error::CoralError;
use crate::process::GitRunner;
use crate::repo::RepoLocation;

/// A name and an address, either of which may be unset.
///
/// `None` is not the empty string. git refuses to commit with an empty `user.email` and falls
/// back to the level above with an absent one, so the two mean opposite things.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub name: Option<String>,
    pub email: Option<String>,
}

impl Identity {
    /// True when this sets neither half and simply inherits.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.name.is_none() && self.email.is_none()
    }
}

/// The identity as it stands for one repository: what git will use, what it inherits, what
/// the repository sets of its own.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct IdentityScopes {
    pub effective: Identity,
    pub global: Identity,
    pub local: Identity,
}

impl RepoLocation {
    /// Reads the identity at every level.
    ///
    /// # Errors
    /// Propagates git failures other than the absent key `git config` reports with exit 1.
    pub async fn identity_scopes(
        &self,
        runner: &GitRunner,
        app: AppConfig<'_>,
    ) -> Result<IdentityScopes, CoralError> {
        Ok(IdentityScopes {
            effective: self.read_identity(runner, Scope::Effective(app)).await?,
            global: self.read_identity(runner, Scope::App(app)).await?,
            local: self.read_identity(runner, Scope::Repository).await?,
        })
    }

    /// Sets the app-level default every repository inherits.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn set_identity_global(
        &self,
        runner: &GitRunner,
        app: AppConfig<'_>,
        who: &Identity,
    ) -> Result<(), CoralError> {
        self.write_identity(runner, Scope::App(app), who).await
    }

    /// Sets what this repository uses. A `None` field clears the override rather than
    /// blanking it, so the level above answers again.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn set_identity_local(
        &self,
        runner: &GitRunner,
        who: &Identity,
    ) -> Result<(), CoralError> {
        self.write_identity(runner, Scope::Repository, who).await
    }

    async fn read_identity(
        &self,
        runner: &GitRunner,
        at: Scope<'_>,
    ) -> Result<Identity, CoralError> {
        Ok(Identity {
            name: none_if_empty(self.config_get(runner, at, "user.name").await?),
            email: none_if_empty(self.config_get(runner, at, "user.email").await?),
        })
    }

    async fn write_identity(
        &self,
        runner: &GitRunner,
        at: Scope<'_>,
        who: &Identity,
    ) -> Result<(), CoralError> {
        self.config_put(runner, at, "user.name", value(who.name.as_deref()))
            .await?;
        self.config_put(runner, at, "user.email", value(who.email.as_deref()))
            .await
    }
}

/// A key set to the empty string reads as unset, since it is what a blanked form field writes
/// and git treats it as no identity at all.
fn none_if_empty(read: Option<String>) -> Option<String> {
    read.filter(|v| !v.trim().is_empty())
}

fn value(field: Option<&str>) -> Option<&str> {
    field.map(str::trim).filter(|v| !v.is_empty())
}
