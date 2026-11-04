//! Where the API key is kept: the OS keyring, beside the hosting tokens, never a file.

use secrecy::{ExposeSecret as _, SecretString};

const SERVICE: &str = "dev.coral.app:anthropic";
const ACCOUNT: &str = "api-key";

fn entry() -> Result<keyring::Entry, crate::DraftError> {
    keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| crate::DraftError::Keyring(e.to_string()))
}

/// The stored key, or `None` when there is none.
///
/// # Errors
/// [`crate::DraftError::Keyring`] when the keyring cannot be reached, which is not the same
/// as there being no key.
pub fn load() -> Result<Option<SecretString>, crate::DraftError> {
    match entry()?.get_password() {
        Ok(secret) => Ok(Some(SecretString::from(secret))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(crate::DraftError::Keyring(e.to_string())),
    }
}

/// Keeps a key, or forgets it with `None`.
///
/// # Errors
/// [`crate::DraftError::Keyring`] when the keyring refuses.
pub fn store(key: Option<&SecretString>) -> Result<(), crate::DraftError> {
    let entry = entry()?;
    let done = match key {
        Some(key) => entry.set_password(key.expose_secret()),
        None => match entry.delete_credential() {
            Err(keyring::Error::NoEntry) => Ok(()),
            other => other,
        },
    };
    done.map_err(|e| crate::DraftError::Keyring(e.to_string()))
}
