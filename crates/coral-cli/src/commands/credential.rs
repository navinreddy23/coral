use std::io::Read as _;

use coral_core::CoralError;
use coral_core::credential::{Action, Keyring, SESSION_VAR, respond, session_matches};

/// Serves one `git credential` invocation on stdin and stdout.
///
/// Git runs this as a subprocess, so the response is written directly rather than through the
/// JSON envelope: git is the consumer, not a person.
///
/// # Errors
/// [`CoralError::Protocol`] if the request does not parse. Everything else — an unknown host,
/// an unauthorised caller — answers empty, because a helper that fails aborts the whole git
/// operation.
pub fn serve(action: &str, session: Option<&str>) -> Result<String, CoralError> {
    let action = Action::parse(action)?;

    let mut request = String::new();
    std::io::stdin().read_to_string(&mut request)?;

    let expected = std::env::var(SESSION_VAR).ok();
    let authorised = session_matches(expected.as_deref(), session);

    respond(&Keyring, action, &request, authorised)
}
