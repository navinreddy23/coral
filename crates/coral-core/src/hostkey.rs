//! ssh's question about a host it has never connected to, put to the user instead of refused.
//!
//! Coral runs ssh with no terminal, so ssh cannot ask "Are you sure you want to continue
//! connecting?" and gives up with "Host key verification failed". Answered here as ssh's
//! `SSH_ASKPASS`, the question is declined and reported; once the user has trusted the key, the
//! same question is answered with its fingerprint, which ssh checks against the key the host
//! actually presented before writing it to its own `known_hosts`. See `SSH_ASKPASS` in ssh(1):
//! <https://man.openbsd.org/ssh#SSH_ASKPASS>.

use std::ffi::OsString;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock, PoisonError};

/// A host key's fingerprint as ssh prints it: `SHA256:…`, or `MD5:…` under `FingerprintHash md5`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
pub struct Fingerprint(String);

impl Fingerprint {
    /// `None` unless `text` has the `ALG:value` shape ssh prints.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (alg, value) = text.split_once(':')?;
        let shaped = !alg.is_empty()
            && alg.bytes().all(|b| b.is_ascii_alphanumeric())
            && !value.is_empty()
            && !value.contains(char::is_whitespace);
        shaped.then(|| Self(text.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A key a host presented that ssh has no record of.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct HostKey {
    /// As ssh names it: `gitlab.com`, or `[git.example.com]:2222` off port 22.
    pub host: String,
    /// `ED25519`, `ECDSA`, `RSA`.
    pub kind: String,
    pub fingerprint: Fingerprint,
}

impl HostKey {
    /// The key ssh's "can't be established … continue connecting?" question is about.
    #[must_use]
    pub fn asked_about(prompt: &str) -> Option<Self> {
        if !prompt.contains("continue connecting") {
            return None;
        }
        let quoted = prompt
            .split_once("The authenticity of host '")?
            .1
            .split_once("' can't be established")?
            .0;
        // "gitlab.com (172.65.251.78)": the name, then the address it resolved to.
        let host = quoted.split(" (").next()?.trim();
        // OpenSSH 10 writes "fingerprint is: SHA256:…", earlier ones "fingerprint is SHA256:….".
        let (kind, rest) = prompt
            .lines()
            .find_map(|line| line.split_once(" key fingerprint is"))?;
        let fingerprint = rest.trim_start_matches(':').trim().trim_end_matches('.');
        Self::new(host, kind.trim(), fingerprint)
    }

    fn new(host: &str, kind: &str, fingerprint: &str) -> Option<Self> {
        let one_word = |s: &str| !s.is_empty() && !s.contains(char::is_whitespace);
        if !one_word(host) || !one_word(kind) {
            return None;
        }
        Some(Self {
            host: host.to_owned(),
            kind: kind.to_owned(),
            fingerprint: Fingerprint::parse(fingerprint)?,
        })
    }
}

/// Set in the environment of every network command, so the binary knows ssh is asking.
pub const ASKPASS_VAR: &str = "CORAL_ASKPASS";
const TRUSTED_VAR: &str = "CORAL_TRUSTED_HOST_KEYS";
const DECLINED: &str = "coral-askpass: declined host key";

struct Askpass {
    binary: PathBuf,
    session: String,
}

static ASKPASS: OnceLock<Askpass> = OnceLock::new();
static TRUSTED: Mutex<Vec<(String, Fingerprint)>> = Mutex::new(Vec::new());

/// Makes `binary` ssh's askpass for every network command from now on.
///
/// `session` stamps the line the helper writes when it declines, so a line a server prints
/// cannot pass for one.
pub fn configure(binary: PathBuf, session: String) {
    let _ = ASKPASS.set(Askpass { binary, session });
}

/// Lets ssh accept `fingerprint` for `host` the next time it asks about it.
pub fn trust(host: &str, fingerprint: Fingerprint) {
    let mut trusted = TRUSTED.lock().unwrap_or_else(PoisonError::into_inner);
    let entry = (host.to_owned(), fingerprint);
    if !trusted.contains(&entry) {
        trusted.push(entry);
    }
}

pub(crate) fn askpass_env() -> Vec<(&'static str, OsString)> {
    let Some(askpass) = ASKPASS.get() else {
        return Vec::new();
    };
    vec![
        ("SSH_ASKPASS", askpass.binary.clone().into_os_string()),
        // `force`: without it ssh only asks a helper when there is a display and no terminal.
        ("SSH_ASKPASS_REQUIRE", "force".into()),
        // git asks `SSH_ASKPASS` for an https user name when `GIT_ASKPASS` is unset, and the
        // helper would answer with a failure of its own; an empty one stops git there.
        ("GIT_ASKPASS", OsString::new()),
        (ASKPASS_VAR, askpass.session.clone().into()),
        (TRUSTED_VAR, trusted_lines().into()),
    ]
}

// `core.askPass` is consulted between `GIT_ASKPASS` and `SSH_ASKPASS`; empty, it stops git there
// even where an empty environment variable does not survive, as on Windows.
pub(crate) fn askpass_args() -> Option<[&'static str; 2]> {
    ASKPASS.get().map(|_| ["-c", "core.askPass="])
}

fn trusted_lines() -> String {
    let trusted = TRUSTED.lock().unwrap_or_else(PoisonError::into_inner);
    trusted
        .iter()
        .fold(String::new(), |mut lines, (host, fingerprint)| {
            let _ = writeln!(lines, "{host} {fingerprint}");
            lines
        })
}

/// The prompt, when this process was started by ssh as its askpass.
///
/// ssh passes the prompt as the only argument. Hooks git runs during a fetch inherit the
/// environment too, but no command a hook could run is a single argument with a space in it.
#[must_use]
pub fn prompt_in(args: &[OsString]) -> Option<String> {
    std::env::var_os(ASKPASS_VAR)?;
    let [_, prompt] = args else { return None };
    let prompt = prompt.to_string_lossy();
    prompt
        .contains(char::is_whitespace)
        .then(|| prompt.into_owned())
}

/// How the helper replies to ssh.
#[derive(Debug, PartialEq, Eq)]
pub enum Answer {
    /// Print the fingerprint: ssh compares it with the presented key and records it.
    Accept(Fingerprint),
    /// Print "no", and write `report` to stderr, where git's caller finds it.
    Decline { report: String },
    /// Not a question about a host key: exit non-zero, as though there were no helper.
    NotOurs,
}

/// Decides the reply to `prompt` from the trust this process's environment carries.
#[must_use]
pub fn answer(prompt: &str) -> Answer {
    let session = std::env::var(ASKPASS_VAR).unwrap_or_default();
    let trusted = std::env::var(TRUSTED_VAR).unwrap_or_default();
    decide(prompt, &session, &trusted)
}

fn decide(prompt: &str, session: &str, trusted: &str) -> Answer {
    let Some(key) = HostKey::asked_about(prompt) else {
        return Answer::NotOurs;
    };
    let approved = trusted.lines().any(|line| {
        line.split_once(' ')
            .is_some_and(|(host, fp)| host == key.host && fp == key.fingerprint.as_str())
    });
    if approved {
        return Answer::Accept(key.fingerprint);
    }
    Answer::Decline {
        report: format!(
            "{DECLINED} {session} {} {} {}",
            key.host, key.kind, key.fingerprint
        ),
    }
}

pub(crate) fn declined_in(stderr: &str) -> Option<HostKey> {
    declined(stderr, &ASKPASS.get()?.session)
}

fn declined(stderr: &str, session: &str) -> Option<HostKey> {
    stderr.lines().find_map(|line| {
        let rest = line.trim().strip_prefix(DECLINED)?;
        let mut words = rest.split_whitespace();
        if words.next()? != session {
            return None;
        }
        HostKey::new(words.next()?, words.next()?, words.next()?)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPENSSH_10: &str = "The authenticity of host '[127.0.0.1]:48231 ([127.0.0.1]:48231)' \
        can't be established.\n\
        ED25519 key fingerprint is: SHA256:aDIVrzG4M+mFkSJjlVuzTna1QcVzJ4oRyqElnLEWsGQ\n\
        This key is not known by any other names.\n\
        Are you sure you want to continue connecting (yes/no/[fingerprint])? ";

    const OPENSSH_9: &str = "The authenticity of host 'gitlab.com (172.65.251.78)' can't be \
        established.\n\
        ED25519 key fingerprint is SHA256:eUXGGm1YGsMAS7vkcx6JOJdOGHPem5gQp4taiCfCLB8.\n\
        This host key is known by the following other names/addresses:\n    \
            ~/.ssh/known_hosts:4: gitlab.example.com\n\
        Are you sure you want to continue connecting (yes/no/[fingerprint])? ";

    fn key(host: &str, kind: &str, fp: &str) -> HostKey {
        HostKey::new(host, kind, fp).unwrap()
    }

    #[test]
    fn reads_the_question_openssh_10_asks() {
        assert_eq!(
            HostKey::asked_about(OPENSSH_10),
            Some(key(
                "[127.0.0.1]:48231",
                "ED25519",
                "SHA256:aDIVrzG4M+mFkSJjlVuzTna1QcVzJ4oRyqElnLEWsGQ"
            ))
        );
    }

    #[test]
    fn reads_the_question_earlier_versions_ask() {
        assert_eq!(
            HostKey::asked_about(OPENSSH_9),
            Some(key(
                "gitlab.com",
                "ED25519",
                "SHA256:eUXGGm1YGsMAS7vkcx6JOJdOGHPem5gQp4taiCfCLB8"
            ))
        );
    }

    #[test]
    fn reads_the_question_with_the_key_drawn_and_hashed_as_md5() {
        let prompt = "The authenticity of host 'git.example.com (10.0.0.2)' can't be \
            established.\n\
            RSA key fingerprint is MD5:9d:1c:6f:3a:2b:4e:5f:60:71:82:93:a4:b5:c6:d7:e8.\n\
            +---[RSA 3072]----+\n|      .o+.       |\n+----[MD5]--------+\n\
            Are you sure you want to continue connecting (yes/no/[fingerprint])? ";
        let asked = HostKey::asked_about(prompt).unwrap();
        assert_eq!(asked.host, "git.example.com");
        assert_eq!(asked.kind, "RSA");
        assert_eq!(
            asked.fingerprint.as_str(),
            "MD5:9d:1c:6f:3a:2b:4e:5f:60:71:82:93:a4:b5:c6:d7:e8"
        );
    }

    /// After a wrong answer ssh asks again in a line that names nothing, and a passphrase or a
    /// password is not a question about a host at all.
    #[test]
    fn leaves_every_other_question_alone() {
        for prompt in [
            "Please type 'yes', 'no' or the fingerprint: ",
            "Enter passphrase for key '/home/dev/.ssh/id_ed25519': ",
            "git@gitlab.com's password: ",
            "Username for 'https://gitlab.com': ",
        ] {
            assert_eq!(decide(prompt, "s", ""), Answer::NotOurs, "{prompt}");
        }
    }

    #[test]
    fn declines_a_key_nobody_has_trusted_and_says_which() {
        assert_eq!(
            decide(OPENSSH_10, "nonce", ""),
            Answer::Decline {
                report: "coral-askpass: declined host key nonce [127.0.0.1]:48231 ED25519 \
                         SHA256:aDIVrzG4M+mFkSJjlVuzTna1QcVzJ4oRyqElnLEWsGQ"
                    .to_owned()
            }
        );
    }

    #[test]
    fn answers_a_trusted_key_with_its_fingerprint() {
        let trusted = "gitlab.com SHA256:other\n\
                       [127.0.0.1]:48231 SHA256:aDIVrzG4M+mFkSJjlVuzTna1QcVzJ4oRyqElnLEWsGQ\n";
        assert_eq!(
            decide(OPENSSH_10, "nonce", trusted),
            Answer::Accept(
                Fingerprint::parse("SHA256:aDIVrzG4M+mFkSJjlVuzTna1QcVzJ4oRyqElnLEWsGQ").unwrap()
            )
        );
    }

    /// Trust is for one key on one host: the same fingerprint elsewhere, or another key here,
    /// is asked about afresh.
    #[test]
    fn trust_does_not_carry_to_another_host_or_another_key() {
        let fp = "SHA256:aDIVrzG4M+mFkSJjlVuzTna1QcVzJ4oRyqElnLEWsGQ";
        for trusted in [
            format!("gitlab.com {fp}"),
            "[127.0.0.1]:48231 SHA256:other".into(),
        ] {
            assert!(
                matches!(decide(OPENSSH_10, "n", &trusted), Answer::Decline { .. }),
                "{trusted}"
            );
        }
    }

    #[test]
    fn finds_the_declined_key_in_what_git_wrote() {
        let stderr = "Cloning into 'project'...\n\
            coral-askpass: declined host key nonce [127.0.0.1]:48231 ED25519 SHA256:abc+/9\r\n\
            Host key verification failed.\n\
            fatal: Could not read from remote repository.";
        assert_eq!(
            declined(stderr, "nonce"),
            Some(key("[127.0.0.1]:48231", "ED25519", "SHA256:abc+/9"))
        );
    }

    /// A server can print anything before git gives up, but it cannot know this process's nonce.
    #[test]
    fn ignores_a_declined_line_from_anyone_else() {
        let stderr = "coral-askpass: declined host key guessed gitlab.com ED25519 SHA256:evil\n\
                      remote: coral-askpass: declined host key nonce gitlab.com ED25519 SHA256:x";
        assert_eq!(declined(stderr, "nonce"), None);
    }

    #[test]
    fn a_fingerprint_is_an_algorithm_and_a_value() {
        assert!(Fingerprint::parse("SHA256:abc+/=").is_some());
        assert!(Fingerprint::parse("MD5:aa:bb").is_some());
        for bad in ["", "SHA256:", ":abc", "SHA 256:abc", "SHA256:a b", "abc"] {
            assert!(Fingerprint::parse(bad).is_none(), "{bad:?}");
        }
    }
}
