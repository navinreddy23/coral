use std::fmt::Write as _;
use std::path::Path;

use coral_core::CoralError;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::signing::{
    AppConfig, SigningFormat, SigningKey, SigningOverrides, SigningScopes, list_keys,
};

/// Reports signing at every level for a repository.
///
/// # Errors
/// Propagates git failures.
pub async fn show(path: &Path) -> Result<SigningScopes, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    loc.signing_scopes(&runner, AppConfig::default()).await
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyList {
    pub keys: Vec<SigningKey>,
}

/// Lists the keys that could sign, for the format in effect here.
///
/// # Errors
/// [`CoralError::Refused`] when the signing program is missing or refuses.
pub async fn keys(path: &Path) -> Result<KeyList, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    let scopes = loc.signing_scopes(&runner, AppConfig::default()).await?;
    Ok(KeyList {
        keys: list_keys(scopes.effective.format, &scopes.effective.program).await?,
    })
}

/// Sets what this repository overrides. An unmentioned setting is left alone; `--inherit`
/// clears every override so the repository follows the app-level defaults again.
///
/// # Errors
/// Propagates git failures.
pub async fn set(
    path: &Path,
    key: Option<String>,
    format: Option<String>,
    sign_commits: Option<bool>,
    inherit: bool,
) -> Result<SigningScopes, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    let app = AppConfig::default();
    let scopes = loc.signing_scopes(&runner, app).await?;

    let overrides = if inherit {
        SigningOverrides::default()
    } else {
        SigningOverrides {
            format: format.as_deref().map(SigningFormat::parse),
            key,
            sign_commits,
            ..scopes.local.clone()
        }
    };
    let for_format = overrides.format.unwrap_or(scopes.effective.format);
    loc.set_signing_local(&runner, &overrides, for_format)
        .await?;
    loc.signing_scopes(&runner, app).await
}

impl crate::output::Human for SigningScopes {
    fn human(&self) -> String {
        let e = &self.effective;
        let mut out = format!(
            "signing is {} here\n  format   {}\n  program  {}\n  key      {}\n  tags     {}",
            if e.sign_commits { "on" } else { "off" },
            e.format.config_value(),
            if e.program.is_empty() {
                format!("{} (git's default)", e.format.default_program())
            } else {
                e.program.clone()
            },
            if e.key.is_empty() {
                "(git chooses)"
            } else {
                &e.key
            },
            if e.sign_tags { "signed" } else { "not signed" },
        );
        if self.local.is_empty() {
            let _ = write!(out, "\n\nall of it inherited from the app-level settings");
        } else {
            let _ = write!(out, "\n\nthis repository overrides:");
            if self.local.format.is_some() {
                let _ = write!(out, " format");
            }
            if self.local.program.is_some() {
                let _ = write!(out, " program");
            }
            if self.local.key.is_some() {
                let _ = write!(out, " key");
            }
            if self.local.sign_commits.is_some() {
                let _ = write!(out, " sign-commits");
            }
            if self.local.sign_tags.is_some() {
                let _ = write!(out, " sign-tags");
            }
            let _ = write!(
                out,
                "\napp level: key {}, signing {}",
                if self.global.key.is_empty() {
                    "(none)"
                } else {
                    &self.global.key
                },
                if self.global.sign_commits {
                    "on"
                } else {
                    "off"
                },
            );
        }
        out
    }
}

impl crate::output::Human for KeyList {
    fn human(&self) -> String {
        if self.keys.is_empty() {
            return "no signing keys".to_owned();
        }
        let mut out = format!("{} keys", self.keys.len());
        for k in &self.keys {
            let _ = write!(
                out,
                "\n  {}{:<42}{}",
                if k.expired { "! " } else { "  " },
                k.id,
                k.label
            );
        }
        out
    }
}
