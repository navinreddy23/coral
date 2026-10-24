use std::io::Read as _;

use coral_core::CoralError;
use coral_core::credential::{Action, Credential, Keyring, SESSION_VAR, respond, session_matches};
use coral_hosting::token::{self, Account};
use coral_hosting::{Host, HostKind};
use secrecy::{ExposeSecret as _, SecretString};

/// Serves one `git credential` invocation on stdin and stdout.
///
/// The application is the binary git spawns, so it answers this itself rather than shelling
/// out to the CLI, which a packaged build has no reason to assume is installed.
///
/// What git has stored is answered first. Failing that, the token the window was signed in to
/// the host with: signing in was for the API alone, so a push over https after it found no
/// credential, and git, told it may not prompt, failed asking for a username.
///
/// # Errors
/// [`CoralError::Protocol`] if the request does not parse. Everything else — an unknown host,
/// an unauthorised caller — answers empty, because a helper that fails aborts the whole git
/// operation rather than falling through to another one.
pub fn serve(action: &str, session: Option<&str>) -> Result<String, CoralError> {
    let action = Action::parse(action)?;

    let mut request = String::new();
    std::io::stdin().read_to_string(&mut request)?;

    let expected = std::env::var(SESSION_VAR).ok();
    if !session_matches(expected.as_deref(), session) {
        return Ok(String::new());
    }
    let signed_in = signed_in(&request);
    match action {
        Action::Get => {
            let stored = respond(&Keyring, action, &request, true)?;
            match signed_in {
                Some((kind, secret)) if stored.is_empty() => answer(&request, kind, secret),
                _ => Ok(stored),
            }
        }
        // git offers back what it was handed, to be kept. The sign-in is kept already, and a
        // second copy here would go on working after signing out.
        Action::Store if signed_in.is_some_and(|(_, secret)| offered(&request, &secret)) => {
            Ok(String::new())
        }
        _ => respond(&Keyring, action, &request, true),
    }
}

/// The token the window signed in to the request's host with: the current profile's own, else
/// the shared one, which is the order the window itself reads them in.
fn signed_in(request: &str) -> Option<(HostKind, SecretString)> {
    let credential = Credential::parse(request).ok()?;
    // Any owner and repository: git names only the host unless `credential.useHttpPath` is on,
    // and the host is all a token is stored under.
    let url = format!(
        "{}://{}/o/r.git",
        credential.protocol()?,
        credential.host()?
    );
    let host = Host::detect(&url).ok()?;
    let accounts = [
        current_profile().map(|id| Account::of_profile(&id)),
        Some(Account::shared()),
    ];
    accounts
        .into_iter()
        .flatten()
        .find_map(|account| token::load(&host, &account).ok().flatten())
        .map(|secret| (host.kind, secret))
}

/// The profile at the window, from the registry it keeps beside its settings.
///
/// Read from disk because the helper is its own process, started by git, and the profile can
/// change while the window runs. Tauri's config directory is `dirs::config_dir()` joined with
/// the application's identifier.
fn current_profile() -> Option<String> {
    let path = dirs::config_dir()?
        .join("dev.coral.app")
        .join("profiles.json");
    let bytes = std::fs::read(path).ok()?;
    let registry: crate::profile::Registry = serde_json::from_slice(&bytes).ok()?;
    Some(registry.current)
}

/// The response for `request` carrying `secret`.
///
/// A token stands in for a password under any user name on both hosts; these are the ones
/// each documents for it, used only when git did not send one of its own.
fn answer(request: &str, kind: HostKind, secret: SecretString) -> Result<String, CoralError> {
    let mut credential = Credential::parse(request)?;
    if credential.username().is_none() {
        let name = match kind {
            HostKind::GitHub => "x-access-token",
            HostKind::GitLab => "oauth2",
        };
        credential.set("username", name);
    }
    credential.set_password(secret);
    Ok(credential.to_response())
}

/// Whether git is offering back exactly `secret`, compared in constant time.
fn offered(request: &str, secret: &SecretString) -> bool {
    let Some(given) = Credential::parse(request)
        .ok()
        .and_then(|c| c.password().cloned())
    else {
        return false;
    };
    let (a, b) = (
        given.expose_secret().as_bytes(),
        secret.expose_secret().as_bytes(),
    );
    a.len() == b.len() && a.iter().zip(b).fold(0_u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::{answer, offered};
    use coral_hosting::HostKind;
    use secrecy::SecretString;

    #[test]
    fn answers_with_the_name_each_host_documents_for_a_token() {
        let github = answer(
            "protocol=https\nhost=github.com\n",
            HostKind::GitHub,
            "t1".into(),
        );
        assert!(github.unwrap().contains("username=x-access-token\n"));
        let gitlab = answer(
            "protocol=https\nhost=gitlab.com\n",
            HostKind::GitLab,
            "t2".into(),
        );
        let gitlab = gitlab.unwrap();
        assert!(gitlab.contains("username=oauth2\n"), "{gitlab}");
        assert!(gitlab.contains("password=t2\n"), "{gitlab}");
    }

    #[test]
    fn keeps_the_user_name_git_sent() {
        let said = answer(
            "protocol=https\nhost=github.com\nusername=ada\n",
            HostKind::GitHub,
            "t".into(),
        )
        .unwrap();
        assert!(said.contains("username=ada\n"), "{said}");
    }

    #[test]
    fn knows_its_own_token_when_git_offers_it_back() {
        let secret = SecretString::from("ghp_secret");
        assert!(offered("host=github.com\npassword=ghp_secret\n", &secret));
        assert!(!offered("host=github.com\npassword=ghp_other\n", &secret));
        assert!(!offered("host=github.com\npassword=ghp_secre\n", &secret));
        assert!(!offered("host=github.com\n", &secret));
    }
}
