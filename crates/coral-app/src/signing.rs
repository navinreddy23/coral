use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::signing::{
    AppConfig, SigningConfig, SigningFormat, SigningKey, SigningOverrides, SigningScopes,
    generate_key, list_keys,
};

use crate::commands::IpcError;

async fn located(path: &str) -> Result<(GitRunner, RepoLocation), IpcError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(path)).await?;
    Ok((runner, loc))
}

/// Signing at every level for a repository.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn signing_read(path: String) -> Result<SigningScopes, IpcError> {
    let (runner, loc) = located(&path).await?;
    Ok(loc.signing_scopes(&runner, AppConfig::default()).await?)
}

/// Sets the app-level defaults every repository inherits.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn signing_set_app(
    path: String,
    config: SigningConfig,
) -> Result<SigningScopes, IpcError> {
    let (runner, loc) = located(&path).await?;
    loc.set_signing_global(&runner, AppConfig::default(), &config)
        .await?;
    Ok(loc.signing_scopes(&runner, AppConfig::default()).await?)
}

/// Sets what this repository overrides. A `null` field clears that override.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn signing_set_repo(
    path: String,
    overrides: SigningOverrides,
) -> Result<SigningScopes, IpcError> {
    let (runner, loc) = located(&path).await?;
    let scopes = loc.signing_scopes(&runner, AppConfig::default()).await?;
    let format = overrides.format.unwrap_or(scopes.effective.format);
    loc.set_signing_local(&runner, &overrides, format).await?;
    Ok(loc.signing_scopes(&runner, AppConfig::default()).await?)
}

/// The keys that could sign, for a format.
///
/// # Errors
/// [`coral_core::CoralError::Refused`] when the program is missing or refuses, which is worth
/// showing: it is usually a program name that is wrong.
#[tauri::command]
pub async fn signing_keys(
    format: SigningFormat,
    program: String,
) -> Result<Vec<SigningKey>, IpcError> {
    Ok(list_keys(format, &program).await?)
}

/// Creates a signing key from the repository's configured identity.
///
/// # Errors
/// [`coral_core::CoralError::Refused`] with gpg's own message, and when the repository has no
/// name or email to put on the key.
#[tauri::command]
pub async fn signing_generate(
    path: String,
    program: String,
    passphrase: String,
) -> Result<SigningKey, IpcError> {
    let (runner, loc) = located(&path).await?;
    let name = loc.config_value(&runner, "user.name").await?;
    let email = loc.config_value(&runner, "user.email").await?;
    Ok(generate_key(
        &program,
        &name,
        &email,
        &secrecy::SecretString::from(passphrase),
    )
    .await?)
}
