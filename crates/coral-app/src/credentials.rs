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
    let host = credential.host()?;
    // Azure DevOps addresses a repository inside a project, and its remotes say so; a
    // placeholder of any other shape is not one its parser takes.
    let path = if host == "dev.azure.com" || host.ends_with("@dev.azure.com") {
        "o/p/_git/r"
    } else if host.ends_with(".visualstudio.com") {
        "p/_git/r"
    } else {
        "o/r.git"
    };
    let url = format!("{}://{host}/{path}", credential.protocol()?);
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
pub(crate) fn current_profile() -> Option<String> {
    let path = dirs::config_dir()?
        .join("dev.coral.app")
        .join("profiles.json");
    let bytes = std::fs::read(path).ok()?;
    let registry: crate::profile::Registry = serde_json::from_slice(&bytes).ok()?;
    Some(registry.current)
}

/// The response for `request` carrying `secret`.
///
/// A token stands in for a password under the user name each host documents for it, used only
/// when git did not send one of its own. Gitea and Azure DevOps take any name at all. A
/// Bitbucket API token belongs to an account and is stored as `user:secret`, so it is split
/// into the two; an access token alone goes under the name Bitbucket reserves for one.
fn answer(request: &str, kind: HostKind, secret: SecretString) -> Result<String, CoralError> {
    let mut credential = Credential::parse(request)?;
    let (user, password) = match (kind, secret.expose_secret().split_once(':')) {
        (HostKind::Bitbucket, Some((user, password))) => (
            Some(user.to_owned()),
            SecretString::from(password.to_owned()),
        ),
        _ => (None, secret),
    };
    if credential.username().is_none() {
        let name = user.unwrap_or_else(|| {
            match kind {
                HostKind::GitHub => "x-access-token",
                HostKind::GitLab => "oauth2",
                HostKind::Bitbucket => "x-token-auth",
                HostKind::Gitea => "git",
                HostKind::Azure => "pat",
            }
            .to_owned()
        });
        credential.set("username", &name);
    }
    credential.set_password(password);
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
    fn the_other_hosts_get_their_names_and_a_bitbucket_api_token_is_split() {
        let said = |kind, secret: &str| {
            answer("protocol=https\nhost=x\n", kind, secret.to_owned().into()).unwrap()
        };
        assert!(said(HostKind::Gitea, "t").contains("username=git\n"));
        assert!(said(HostKind::Azure, "t").contains("username=pat\n"));
        let access = said(HostKind::Bitbucket, "ATCTT");
        assert!(access.contains("username=x-token-auth\n"), "{access}");
        assert!(access.contains("password=ATCTT\n"), "{access}");
        let api = said(HostKind::Bitbucket, "ada@x.invalid:ATATT");
        assert!(api.contains("username=ada@x.invalid\n"), "{api}");
        assert!(api.contains("password=ATATT\n"), "{api}");
        assert!(!api.contains("ada@x.invalid:ATATT"), "{api}");
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
