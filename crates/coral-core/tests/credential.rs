//! The credential helper. The nonce check is the entire defence against another process on the
//! machine invoking the coral binary to read the user's tokens, so it is tested hardest.

use std::cell::RefCell;
use std::collections::HashMap;

use coral_core::credential::{
    Action, Credential, CredentialStore, helper_args, respond, session_matches,
};
use secrecy::{ExposeSecret as _, SecretString};

/// An in-memory store, so tests never touch the developer's real keyring.
#[derive(Default)]
struct Memory {
    entries: RefCell<HashMap<(String, String), String>>,
}

impl Memory {
    fn seed(&self, key: &str, user: &str, secret: &str) {
        self.entries
            .borrow_mut()
            .insert((key.to_owned(), user.to_owned()), secret.to_owned());
    }

    fn len(&self) -> usize {
        self.entries.borrow().len()
    }
}

impl CredentialStore for Memory {
    fn get(
        &self,
        key: &str,
        username: &str,
    ) -> Result<Option<SecretString>, coral_core::CoralError> {
        Ok(self
            .entries
            .borrow()
            .get(&(key.to_owned(), username.to_owned()))
            .map(|s| SecretString::from(s.clone())))
    }

    fn set(
        &self,
        key: &str,
        username: &str,
        secret: &SecretString,
    ) -> Result<(), coral_core::CoralError> {
        self.seed(key, username, secret.expose_secret());
        Ok(())
    }

    fn delete(&self, key: &str, username: &str) -> Result<(), coral_core::CoralError> {
        self.entries
            .borrow_mut()
            .remove(&(key.to_owned(), username.to_owned()));
        Ok(())
    }
}

const REQUEST: &str = "protocol=https\nhost=github.com\nusername=navin\n";

#[test]
fn parses_the_request_git_writes() {
    let c = Credential::parse(REQUEST).unwrap();
    assert_eq!(c.protocol(), Some("https"));
    assert_eq!(c.host(), Some("github.com"));
    assert_eq!(c.username(), Some("navin"));
    assert!(c.password().is_none());
}

#[test]
fn stops_at_the_blank_line_that_ends_a_request() {
    let c = Credential::parse("protocol=https\nhost=a.com\n\nhost=evil.com\n").unwrap();
    assert_eq!(
        c.host(),
        Some("a.com"),
        "anything after the blank line is not ours to read"
    );
}

#[test]
fn rejects_a_line_that_is_not_key_equals_value() {
    assert!(Credential::parse("protocol=https\ngarbage\n").is_err());
}

/// Git only sends `path` when credential.useHttpPath is set, so its absence must not change
/// the key for a host that never sends it.
#[test]
fn the_storage_key_is_stable_and_includes_the_path_only_when_sent() {
    let plain = Credential::parse("protocol=https\nhost=github.com\n").unwrap();
    assert_eq!(plain.storage_key(), "https://github.com");

    let with_path = Credential::parse("protocol=https\nhost=github.com\npath=o/r.git\n").unwrap();
    assert_eq!(with_path.storage_key(), "https://github.com/o/r.git");
}

#[test]
fn a_get_returns_the_stored_secret() {
    let store = Memory::default();
    store.seed("https://github.com", "navin", "ghp_token");

    let out = respond(&store, Action::Get, REQUEST, true).unwrap();
    assert!(out.contains("password=ghp_token"));
    assert!(out.contains("username=navin"));
}

/// This is the attack the nonce exists to stop: any process can run the coral binary.
#[test]
fn an_unauthorised_get_returns_nothing_at_all() {
    let store = Memory::default();
    store.seed("https://github.com", "navin", "ghp_token");

    let out = respond(&store, Action::Get, REQUEST, false).unwrap();
    assert!(
        out.is_empty(),
        "an unauthorised caller must learn nothing, not even the username"
    );
    assert!(!out.contains("ghp_token"));
}

#[test]
fn an_unauthorised_store_writes_nothing() {
    let store = Memory::default();
    let request = format!("{REQUEST}password=injected\n");

    respond(&store, Action::Store, &request, false).unwrap();
    assert_eq!(
        store.len(),
        0,
        "an unauthorised caller must not be able to plant a credential"
    );
}

#[test]
fn an_unauthorised_erase_deletes_nothing() {
    let store = Memory::default();
    store.seed("https://github.com", "navin", "ghp_token");

    respond(&store, Action::Erase, REQUEST, false).unwrap();
    assert_eq!(
        store.len(),
        1,
        "an unauthorised caller must not be able to lock the user out"
    );
}

#[test]
fn store_then_get_then_erase_round_trips() {
    let store = Memory::default();
    let request = format!("{REQUEST}password=secret123\n");

    respond(&store, Action::Store, &request, true).unwrap();
    // Two entries: the credential, and the username filed against the host so a later request
    // that carries none can still be answered.
    assert_eq!(store.len(), 2);

    let got = respond(&store, Action::Get, REQUEST, true).unwrap();
    assert!(got.contains("password=secret123"));

    respond(&store, Action::Erase, REQUEST, true).unwrap();
    assert_eq!(store.len(), 0);
    assert!(
        respond(&store, Action::Get, REQUEST, true)
            .unwrap()
            .is_empty()
    );
}

/// A helper that errors aborts the whole git operation, so "I have nothing" must be an empty
/// response rather than a failure.
#[test]
fn an_unknown_host_answers_empty_rather_than_failing() {
    let store = Memory::default();
    let out = respond(
        &store,
        Action::Get,
        "protocol=https\nhost=unknown.example\nusername=x\n",
        true,
    );
    assert_eq!(out.unwrap(), "");
}

#[test]
fn a_request_without_a_username_answers_empty() {
    let store = Memory::default();
    store.seed("https://github.com", "navin", "ghp_token");

    let out = respond(
        &store,
        Action::Get,
        "protocol=https\nhost=github.com\n",
        true,
    )
    .unwrap();
    assert!(out.is_empty(), "there is no account to look up yet");
}

/// A missing or empty expectation must deny. A helper that answers when unconfigured is worse
/// than one that never answers.
#[test]
fn the_session_check_denies_unless_both_sides_match_exactly() {
    assert!(session_matches(Some("abc123"), Some("abc123")));

    assert!(!session_matches(Some("abc123"), Some("abc124")));
    assert!(!session_matches(Some("abc123"), None));
    assert!(!session_matches(None, Some("abc123")));
    assert!(!session_matches(None, None));
    assert!(
        !session_matches(Some(""), Some("")),
        "an empty nonce is not a match"
    );
    assert!(
        !session_matches(Some("abc123"), Some("abc")),
        "a prefix is not a match"
    );
    assert!(
        !session_matches(Some("abc"), Some("abc123")),
        "nor is an extension"
    );
}

/// The empty value first is what makes ours authoritative; without it the user's own helper
/// could answer first and ours would never be consulted.
#[test]
fn the_helper_arguments_clear_existing_helpers_first() {
    let args = helper_args(std::path::Path::new("/usr/local/bin/coral"), "n0nce");

    assert_eq!(args[0], "-c");
    assert_eq!(
        args[1], "credential.helper=",
        "the empty value clears inherited helpers"
    );
    assert_eq!(args[2], "-c");
    assert!(args[3].contains("/usr/local/bin/coral"));
    assert!(args[3].contains("credential-helper"));
    // Without the nonce on the command line the helper answers empty every time, because
    // there is nothing for it to compare the environment against.
    assert!(args[3].contains("--session 'n0nce'"));
    // git appends the action, so the nonce must come before it.
    assert!(args[3].trim_end().ends_with("'n0nce'"));
}

/// The password must never reach a log through an accidental Debug format.
#[test]
fn the_debug_rendering_hides_the_password() {
    let c = Credential::parse(&format!("{REQUEST}password=ghp_supersecret\n")).unwrap();
    let rendered = format!("{c:?}");

    assert!(
        !rendered.contains("ghp_supersecret"),
        "Debug leaked the secret: {rendered}"
    );
    assert!(
        rendered.contains("github.com"),
        "the non-secret fields are still useful"
    );
}

#[test]
fn actions_parse_and_unknown_ones_are_refused() {
    assert_eq!(Action::parse("get").unwrap(), Action::Get);
    assert_eq!(Action::parse("store").unwrap(), Action::Store);
    assert_eq!(Action::parse("erase").unwrap(), Action::Erase);

    let err = Action::parse("exfiltrate").unwrap_err();
    assert_eq!(err.code(), "refused");
}

#[test]
fn a_network_command_carries_the_helper_and_its_nonce() {
    use coral_core::process::GitCommand;

    // The nonce goes on the command line and in the environment; the helper answers only when
    // they match, which is what stops another local process asking for the user's tokens.
    coral_core::credential::configure(std::path::PathBuf::from("/opt/coral"), "abc123".to_owned());

    let network = GitCommand::network("push", "/tmp").args(["push", "origin"]);
    let argv = network.redacted_argv(std::path::Path::new("git")).join(" ");
    assert!(
        argv.contains("credential.helper="),
        "clears inherited helpers"
    );
    assert!(argv.contains("/opt/coral"), "points at this binary: {argv}");
    assert!(
        argv.contains("--session 'abc123'"),
        "carries the nonce: {argv}"
    );

    // A read must not be able to reach it: a helper answering a request the user never saw a
    // prompt for is a credential leak, not a convenience.
    let read = GitCommand::read("rev-list", "/tmp").args(["rev-list", "HEAD"]);
    let read_argv = read.redacted_argv(std::path::Path::new("git")).join(" ");
    assert!(!read_argv.contains("credential.helper"), "{read_argv}");
}

#[test]
fn a_request_with_no_username_is_answered_from_the_one_last_stored() {
    // A token push sends only the protocol and host, because the remote URL carries no
    // username. Answering nothing there sends git to ask for one on a terminal it has been
    // told it cannot use, and the push fails against a prompt nobody sees.
    let store = Memory::default();
    respond(
        &store,
        Action::Store,
        "protocol=https\nhost=github.com\nusername=alice\npassword=ghp_token\n\n",
        true,
    )
    .unwrap();

    let answer = respond(
        &store,
        Action::Get,
        "protocol=https\nhost=github.com\n\n",
        true,
    )
    .unwrap();

    assert!(answer.contains("username=alice"), "{answer}");
    assert!(answer.contains("password=ghp_token"), "{answer}");
}

#[test]
fn a_username_in_the_request_still_wins_over_the_remembered_one() {
    let store = Memory::default();
    for (user, secret) in [("alice", "one"), ("bob", "two")] {
        respond(
            &store,
            Action::Store,
            &format!("protocol=https\nhost=github.com\nusername={user}\npassword={secret}\n\n"),
            true,
        )
        .unwrap();
    }
    // bob was stored last and is what a bare request would get; asking for alice must not.
    let answer = respond(
        &store,
        Action::Get,
        "protocol=https\nhost=github.com\nusername=alice\n\n",
        true,
    )
    .unwrap();
    assert!(answer.contains("password=one"), "{answer}");
}

#[test]
fn erasing_forgets_the_remembered_name_as_well() {
    // Otherwise the next bare request answers with a user whose password has just been
    // deleted, and git reports a wrong password rather than a missing one.
    let store = Memory::default();
    respond(
        &store,
        Action::Store,
        "protocol=https\nhost=github.com\nusername=alice\npassword=ghp_token\n\n",
        true,
    )
    .unwrap();
    respond(
        &store,
        Action::Erase,
        "protocol=https\nhost=github.com\nusername=alice\n\n",
        true,
    )
    .unwrap();

    let answer = respond(
        &store,
        Action::Get,
        "protocol=https\nhost=github.com\n\n",
        true,
    )
    .unwrap();
    assert_eq!(answer, "");
}

#[test]
fn the_remembered_name_is_kept_per_host() {
    let store = Memory::default();
    for (host, user) in [("github.com", "alice"), ("gitlab.com", "bob")] {
        respond(
            &store,
            Action::Store,
            &format!("protocol=https\nhost={host}\nusername={user}\npassword=x\n\n"),
            true,
        )
        .unwrap();
    }
    let answer = respond(
        &store,
        Action::Get,
        "protocol=https\nhost=github.com\n\n",
        true,
    )
    .unwrap();
    assert!(answer.contains("username=alice"), "{answer}");
}

#[test]
fn an_unauthorised_caller_gets_nothing_even_when_a_name_is_remembered() {
    let store = Memory::default();
    respond(
        &store,
        Action::Store,
        "protocol=https\nhost=github.com\nusername=alice\npassword=ghp_token\n\n",
        true,
    )
    .unwrap();
    let answer = respond(
        &store,
        Action::Get,
        "protocol=https\nhost=github.com\n\n",
        false,
    )
    .unwrap();
    assert_eq!(answer, "", "the nonce check must come before the lookup");
}
