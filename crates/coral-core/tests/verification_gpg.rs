//! git's verdict on gpg signatures, with a keyring made for the test.
//!
//! git and gpg are pointed at the throwaway keyring through `gpg.program`, a two-line wrapper
//! that sets `GNUPGHOME`: the runner takes no environment of its own, and nothing here may read
//! or write the keyring of whoever runs the tests.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;
use coral_core::verification::{CHECK_LIMIT, Verification};

/// A keyring in a temporary directory, its agent stopped when it goes.
struct Keyring {
    home: tempfile::TempDir,
}

impl Keyring {
    fn empty() -> Self {
        let home = tempfile::tempdir().unwrap();
        std::fs::set_permissions(home.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        Self { home }
    }

    fn with_key() -> Self {
        let ring = Self::empty();
        let made = Command::new("gpg")
            .env("GNUPGHOME", ring.home.path())
            .args([
                "--batch",
                "--quiet",
                "--pinentry-mode",
                "loopback",
                "--passphrase",
                "",
            ])
            .args([
                "--quick-gen-key",
                "Fixture <fixture@coral.test>",
                "ed25519",
                "sign",
                "never",
            ])
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(made.success());
        ring
    }

    /// A program for `gpg.program` that runs gpg on this keyring.
    fn program(&self) -> PathBuf {
        script(
            self.home.path(),
            &format!("GNUPGHOME='{}' exec gpg \"$@\"", self.home.path().display()),
        )
    }
}

impl Drop for Keyring {
    /// Stops the agent, then takes away the socket directory gpg made for this home under
    /// `/run/user`, which would otherwise outlive the test by one empty directory a run.
    fn drop(&mut self) {
        for args in [["--kill", "gpg-agent"], ["--remove-socketdir", ""]] {
            let _ = Command::new("gpgconf")
                .env("GNUPGHOME", self.home.path())
                .args(args.iter().filter(|a| !a.is_empty()))
                .stderr(Stdio::null())
                .status();
        }
    }
}

fn script(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("program");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn gpg_present() -> bool {
    Command::new("gpg")
        .arg("--version")
        .stdout(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// A repository whose `HEAD` is signed with `ring`'s key, read through `ring` too.
fn signed(ring: &Keyring) -> TestRepo {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("start");
    repo.git(["config", "gpg.format", "openpgp"]);
    repo.git(["config", "gpg.program", ring.program().to_str().unwrap()]);
    repo.git(["config", "user.signingkey", "fixture@coral.test"]);
    let repo = repo.write("a.txt", "2\n");
    repo.git(["add", "a.txt"]);
    repo.git(["commit", "-q", "-S", "-m", "signed"]);
    repo
}

async fn verdict(repo: &TestRepo, rev: &str, limit: Duration) -> Verification {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    loc.verification(&runner, rev, limit).await.unwrap()
}

#[tokio::test]
async fn a_signature_from_a_key_in_the_keyring_is_good() {
    if !gpg_present() {
        return;
    }
    let ring = Keyring::with_key();
    let repo = signed(&ring);
    assert_eq!(
        verdict(&repo, "HEAD", CHECK_LIMIT).await,
        Verification::Good {
            signer: "Fixture <fixture@coral.test>".into(),
            trusted: true
        }
    );
}

#[tokio::test]
async fn a_key_this_keyring_has_never_seen_is_unchecked_and_says_so() {
    if !gpg_present() {
        return;
    }
    let ring = Keyring::with_key();
    let repo = signed(&ring);
    let stranger = Keyring::empty();
    repo.git([
        "config",
        "gpg.program",
        stranger.program().to_str().unwrap(),
    ]);
    match verdict(&repo, "HEAD", CHECK_LIMIT).await {
        Verification::Unchecked { reason } => assert!(reason.contains("No public key"), "{reason}"),
        other => panic!("expected unchecked, got {other:?}"),
    }
}

#[tokio::test]
async fn a_commit_altered_after_signing_is_bad() {
    if !gpg_present() {
        return;
    }
    let ring = Keyring::with_key();
    let repo = signed(&ring);
    // The same signature over a different message: what a forged commit looks like.
    let raw = repo.git(["cat-file", "commit", "HEAD"]);
    // `git` hands back its output trimmed, so the message has no newline after it here.
    let forged = format!("{}\n", raw.replace("\n\nsigned", "\n\nforged"));
    assert_ne!(raw, forged);
    let mut hash = Command::new("git")
        .current_dir(repo.path())
        .args(["hash-object", "-t", "commit", "-w", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(hash.stdin.as_mut().unwrap(), forged.as_bytes()).unwrap();
    let out = hash.wait_with_output().unwrap();
    let oid = String::from_utf8(out.stdout).unwrap();
    assert!(matches!(
        verdict(&repo, oid.trim(), CHECK_LIMIT).await,
        Verification::Bad { .. }
    ));
}

#[tokio::test]
async fn a_signing_program_that_never_answers_is_given_up_on() {
    if !gpg_present() {
        return;
    }
    let ring = Keyring::with_key();
    let repo = signed(&ring);
    let stuck = tempfile::tempdir().unwrap();
    let program = script(stuck.path(), "sleep 30");
    repo.git(["config", "gpg.program", program.to_str().unwrap()]);
    let started = std::time::Instant::now();
    match verdict(&repo, "HEAD", Duration::from_secs(1)).await {
        Verification::Unchecked { reason } => {
            assert!(reason.contains("did not answer"), "{reason}");
        }
        other => panic!("expected unchecked, got {other:?}"),
    }
    assert!(started.elapsed() < Duration::from_secs(10));
}
