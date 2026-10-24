//! Reading git's verdict on a commit's signature, with a throwaway SSH key.

#![cfg(unix)]

use std::process::Command;

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;
use coral_core::verification::{CHECK_LIMIT, Verification, carries_signature};

/// A repository whose `HEAD` is signed by a key made for the test, and the key's public half.
fn signed() -> (TestRepo, tempfile::TempDir, String) {
    let keys = tempfile::tempdir().unwrap();
    let key = keys.path().join("key");
    let made = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-C", "test", "-f"])
        .arg(&key)
        .status()
        .unwrap();
    assert!(made.success());
    let public = std::fs::read_to_string(key.with_extension("pub")).unwrap();

    let repo = TestRepo::new().write("a.txt", "1\n").commit("unsigned");
    repo.git(["config", "gpg.format", "ssh"]);
    // Named here because a developer's own global config may name another, or none at all.
    repo.git(["config", "gpg.ssh.program", "ssh-keygen"]);
    repo.git(["config", "user.signingkey", key.to_str().unwrap()]);
    let repo = repo.write("a.txt", "2\n");
    repo.git(["add", "a.txt"]);
    repo.git(["commit", "-q", "-S", "-m", "signed"]);
    (repo, keys, public)
}

async fn verdict(repo: &TestRepo, rev: &str) -> Verification {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    loc.verification(&runner, rev, CHECK_LIMIT).await.unwrap()
}

#[tokio::test]
async fn an_unsigned_commit_says_so() {
    let (repo, _keys, _) = signed();
    assert_eq!(verdict(&repo, "HEAD~1").await, Verification::Unsigned);
}

#[tokio::test]
async fn a_valid_signature_from_no_allowed_signer_is_good_but_not_trusted() {
    let (repo, _keys, _) = signed();
    // Pointed at a file that is not there, whatever the developer's own config says. git
    // still checks the signature itself, and cannot say whose it is.
    repo.git([
        "config",
        "gpg.ssh.allowedSignersFile",
        "/nonexistent/allowed",
    ]);
    assert_eq!(
        verdict(&repo, "HEAD").await,
        Verification::Good {
            signer: String::new(),
            trusted: false
        }
    );
}

#[test]
fn a_signed_commit_carries_its_signature_header() {
    let (repo, _keys, _) = signed();
    assert!(carries_signature(
        repo.git(["cat-file", "commit", "HEAD"]).as_bytes()
    ));
    assert!(!carries_signature(
        repo.git(["cat-file", "commit", "HEAD~1"]).as_bytes()
    ));
}

#[tokio::test]
async fn a_signature_from_an_allowed_signer_is_good() {
    let (repo, keys, public) = signed();
    let allowed = keys.path().join("allowed");
    std::fs::write(&allowed, format!("fixture@coral.test {public}")).unwrap();
    repo.git([
        "config",
        "gpg.ssh.allowedSignersFile",
        allowed.to_str().unwrap(),
    ]);
    assert!(matches!(
        verdict(&repo, "HEAD").await,
        Verification::Good { .. }
    ));
}
