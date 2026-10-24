//! Whether a commit's signature holds, as git itself judges it.
//!
//! Git does the checking, through the same program and trust settings it signs with, and
//! reports a one-letter verdict (`%G?`, see `git help log` under "PRETTY FORMATS"). The letter
//! alone misleads in two cases, which is why git's stderr is read beside it:
//!
//! - an SSH signature with no `gpg.ssh.allowedSignersFile` comes back `N`, "no signature";
//! - a signing program that cannot be run comes back `B`, "bad signature".
//!
//! Both are signatures nothing here could check, and git's message says what is missing.

use std::time::Duration;

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;

/// What git makes of a commit's signature.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Verification {
    Unsigned,
    /// A good signature. `trusted` is false when the key is not one git has been told to trust.
    Good {
        signer: String,
        trusted: bool,
    },
    Bad {
        signer: String,
    },
    /// Good when made, but the signature or its key has since expired.
    Expired {
        signer: String,
    },
    Revoked {
        signer: String,
    },
    /// Signed, but git could not check it here. `reason` is git's own message, or empty.
    Unchecked {
        reason: String,
    },
}

impl Verification {
    /// Reads `%G?%x00%GS%x00%GG`, with git's stderr from the same run and whether the commit
    /// carries a signature at all.
    ///
    /// The reason for an unchecked signature is git's own complaint when it made one, and
    /// otherwise the signing program's last word in `%GG`: gpg says "No public key" there and
    /// leaves git's stderr empty.
    #[must_use]
    pub fn judge(stdout: &[u8], stderr: &[u8], has_signature: bool) -> Self {
        let text = String::from_utf8_lossy(stdout);
        let mut parts = text.trim_end_matches('\n').splitn(3, '\0');
        let code = parts.next().unwrap_or("");
        let signer = parts.next().unwrap_or("").to_owned();
        let said = parts.next().unwrap_or("");
        let reason = match first_error(stderr) {
            complaint if !complaint.is_empty() => complaint,
            _ => last_word(said),
        };
        match code {
            "G" => Self::Good {
                signer,
                trusted: true,
            },
            "U" => Self::Good {
                signer,
                trusted: false,
            },
            "B" if reason.contains("cannot run") => Self::Unchecked { reason },
            "B" => Self::Bad { signer },
            "X" | "Y" => Self::Expired { signer },
            "R" => Self::Revoked { signer },
            "E" => Self::Unchecked { reason },
            _ if has_signature => Self::Unchecked { reason },
            _ => Self::Unsigned,
        }
    }
}

/// Whether a raw commit object has a signature header, before the blank line that ends them.
#[must_use]
pub fn carries_signature(commit: &[u8]) -> bool {
    commit
        .split(|b| *b == b'\n')
        .take_while(|line| !line.is_empty())
        .any(|line| line.starts_with(b"gpgsig"))
}

/// The last line the signing program wrote, which is where gpg gives its verdict.
fn last_word(said: &str) -> String {
    let line = said
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .unwrap_or("");
    line.strip_prefix("gpg: ").unwrap_or(line).to_owned()
}

/// The first line git or the signing program flagged, without its `error: ` or `gpg: ` prefix.
fn first_error(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    line.strip_prefix("error: ")
        .or_else(|| line.strip_prefix("gpg: "))
        .unwrap_or(line)
        .to_owned()
}

/// How long a signature check may take before it is reported as unchecked.
///
/// The signing program runs for every signed commit looked at, and gpg can be set to fetch a
/// key it does not know from a key server (`auto-key-retrieve`), or wait on an agent that is
/// not answering. Neither should hold up the panel for the read limit of thirty seconds.
pub const CHECK_LIMIT: Duration = Duration::from_secs(5);

impl RepoLocation {
    /// Asks git to check one commit's signature, giving up after `limit`.
    ///
    /// Only ever for one commit at a time: git runs the signing program for each signed commit
    /// it formats this way, which is far too slow for a graph.
    ///
    /// # Errors
    /// Propagates git failures, including an unknown revision. Running out of time is not one:
    /// it is a signature that could not be checked, and says so.
    pub async fn verification(
        &self,
        runner: &GitRunner,
        rev: &str,
        limit: Duration,
    ) -> Result<Verification, CoralError> {
        match tokio::time::timeout(limit, self.check(runner, rev)).await {
            Ok(answer) => answer,
            Err(_) => Ok(Verification::Unchecked {
                reason: format!(
                    "the signing program did not answer within {} seconds",
                    limit.as_secs()
                ),
            }),
        }
    }

    async fn check(&self, runner: &GitRunner, rev: &str) -> Result<Verification, CoralError> {
        let verdict = runner
            .output(GitCommand::read("log", self.display_path()).args([
                "log",
                "-1",
                "--format=%G?%x00%GS%x00%GG",
                rev,
                "--",
            ]))
            .await?;
        let raw = runner
            .output(
                GitCommand::read("cat-file", self.display_path()).args(["cat-file", "commit", rev]),
            )
            .await?;
        Ok(Verification::judge(
            &verdict.stdout,
            &verdict.stderr,
            carries_signature(&raw.stdout),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{Verification, carries_signature};

    fn signer(name: &str) -> String {
        name.to_owned()
    }

    #[test]
    fn reads_each_verdict() {
        let judge = |out: &[u8]| Verification::judge(out, b"", true);
        assert_eq!(
            judge(b"G\0Ada <ada@example.test>\n"),
            Verification::Good {
                signer: signer("Ada <ada@example.test>"),
                trusted: true
            }
        );
        assert_eq!(
            judge(b"U\0Ada\n"),
            Verification::Good {
                signer: signer("Ada"),
                trusted: false
            }
        );
        assert_eq!(
            judge(b"B\0Eve\n"),
            Verification::Bad {
                signer: signer("Eve")
            }
        );
        assert_eq!(
            judge(b"Y\0Ada\n"),
            Verification::Expired {
                signer: signer("Ada")
            }
        );
        assert_eq!(
            judge(b"R\0Ada\n"),
            Verification::Revoked {
                signer: signer("Ada")
            }
        );
    }

    #[test]
    fn a_commit_without_a_signature_is_unsigned() {
        assert_eq!(
            Verification::judge(b"N\0\n", b"", false),
            Verification::Unsigned
        );
        assert_eq!(Verification::judge(b"", b"", false), Verification::Unsigned);
    }

    #[test]
    fn an_ssh_signature_with_no_allowed_signers_is_unchecked_not_unsigned() {
        let stderr = b"error: gpg.ssh.allowedSignersFile needs to be configured and exist for ssh signature verification\n";
        assert_eq!(
            Verification::judge(b"N\0\n", stderr, true),
            Verification::Unchecked {
                reason: "gpg.ssh.allowedSignersFile needs to be configured and exist for ssh signature verification".into()
            }
        );
    }

    #[test]
    fn a_program_that_cannot_run_is_unchecked_not_bad() {
        let stderr = b"error: cannot run : No such file or directory\nerror: cannot run : No such file or directory\n";
        assert_eq!(
            Verification::judge(b"B\0\n", stderr, true),
            Verification::Unchecked {
                reason: "cannot run : No such file or directory".into()
            }
        );
    }

    #[test]
    fn a_missing_key_says_why_in_gpgs_words_when_git_says_nothing() {
        let out = b"E\0\0gpg: Signature made Sat 24 Oct\ngpg:                issuer \"f@x.test\"\ngpg: Can't check signature: No public key\n\n";
        assert_eq!(
            Verification::judge(out, b"", true),
            Verification::Unchecked {
                reason: "Can't check signature: No public key".into()
            }
        );
    }

    #[test]
    fn a_missing_key_says_why() {
        assert_eq!(
            Verification::judge(
                b"E\0\n",
                b"gpg: Can't check signature: No public key\n",
                true
            ),
            Verification::Unchecked {
                reason: "Can't check signature: No public key".into()
            }
        );
    }

    #[test]
    fn finds_the_signature_header_only_among_the_headers() {
        assert!(carries_signature(
            b"tree a\nauthor x\ngpgsig -----BEGIN SSH SIGNATURE-----\n more\n\nmsg\n"
        ));
        assert!(!carries_signature(
            b"tree a\nauthor x\n\ngpgsig in the message\n"
        ));
    }
}
