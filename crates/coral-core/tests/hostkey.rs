//! ssh's question about a new host reaches Coral's askpass from network commands, and only those.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use coral_core::CoralError;
use coral_core::hostkey::{self, Fingerprint};
use coral_core::process::{GitCommand, GitRunner};

const SESSION: &str = "test-session";

fn run<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Runtime::new().unwrap().block_on(f)
}

fn script(at: &Path, body: &str) -> PathBuf {
    std::fs::write(at, format!("#!/bin/sh\n{body}")).unwrap();
    std::fs::set_permissions(at, std::fs::Permissions::from_mode(0o755)).unwrap();
    at.to_path_buf()
}

// One askpass per process, as in the front ends: it stands in for theirs by declining the way
// `hostkey::answer` does, which the unit tests cover.
fn scratch() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("coral-hostkey-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let askpass = script(
            &dir.join("askpass"),
            "echo \"coral-askpass: declined host key $CORAL_ASKPASS [h.test]:22 ED25519 \
             SHA256:abc+/9\" >&2\necho no\n",
        );
        hostkey::configure(askpass, SESSION.to_owned());
        dir
    })
}

// What ssh does with no terminal: put the question to its askpass and give up on "no".
fn fake_ssh(name: &str, also: &str) -> String {
    let at = script(
        &scratch().join(name),
        &format!(
            "{also}\nanswer=$(\"$SSH_ASKPASS\" \"The authenticity of host '[h.test]:22 \
             ([10.0.0.2]:22)' can't be established.\nED25519 key fingerprint is: \
             SHA256:abc+/9\nAre you sure you want to continue connecting \
             (yes/no/[fingerprint])? \")\n\
             [ \"$answer\" = no ] && echo 'Host key verification failed.' >&2\nexit 255\n"
        ),
    );
    at.display().to_string()
}

async fn env_of(cmd: GitCommand) -> String {
    let runner = GitRunner::discover().await.unwrap();
    let out = runner
        .output(cmd.args(["-c", "alias.probe=!env", "probe"]))
        .await
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn a_network_command_lets_ssh_ask_coral_and_a_read_does_not() {
    let dir = scratch();
    let network = run(env_of(GitCommand::network("probe", dir)));
    let askpass = dir.join("askpass").display().to_string();
    for wanted in [
        format!("SSH_ASKPASS={askpass}"),
        "SSH_ASKPASS_REQUIRE=force".to_owned(),
        format!("CORAL_ASKPASS={SESSION}"),
        "GIT_ASKPASS=".to_owned(),
    ] {
        assert!(
            network.lines().any(|l| l == wanted),
            "{wanted} in\n{network}"
        );
    }
    assert!(
        network.to_lowercase().contains("'core.askpass'=''"),
        "git's own askpass setting is emptied too:\n{network}"
    );

    let read = run(env_of(GitCommand::read("probe", dir)));
    assert!(
        read.lines().any(|l| l == "SSH_ASKPASS_REQUIRE=never"),
        "{read}"
    );
    assert!(!read.contains("CORAL_ASKPASS="), "{read}");
}

#[test]
fn a_host_nobody_has_trusted_is_reported_with_its_key() {
    let dir = scratch();
    let cmd = GitCommand::network("ls-remote", dir)
        .args(["ls-remote", "ssh://h.test/project.git"])
        .env("GIT_SSH_COMMAND", fake_ssh("ssh-decline", ""));
    let failed = run(async {
        let runner = GitRunner::discover().await.unwrap();
        runner
            .output(cmd)
            .await
            .err()
            .expect("the fake ssh gives up")
    });
    let CoralError::UnknownHostKey {
        host_key, stderr, ..
    } = &failed
    else {
        panic!("expected an unknown host key, got {failed:?}");
    };
    assert_eq!(failed.code(), "unknown_host_key");
    assert_eq!(host_key.host, "[h.test]:22");
    assert_eq!(host_key.kind, "ED25519");
    assert_eq!(host_key.fingerprint.as_str(), "SHA256:abc+/9");
    assert!(stderr.contains("Host key verification failed."), "{stderr}");
    assert!(
        !stderr.contains(SESSION),
        "the nonce stays out of the message: {stderr}"
    );
}

#[test]
fn a_trusted_key_travels_to_the_askpass_ssh_runs() {
    let dir = scratch();
    hostkey::trust("[h.test]:22", Fingerprint::parse("SHA256:abc+/9").unwrap());
    let seen = dir.join("trusted-seen");
    let cmd = GitCommand::network("ls-remote", dir)
        .args(["ls-remote", "ssh://h.test/project.git"])
        .env(
            "GIT_SSH_COMMAND",
            fake_ssh(
                "ssh-record",
                &format!(
                    "printf '%s' \"$CORAL_TRUSTED_HOST_KEYS\" > '{}'",
                    seen.display()
                ),
            ),
        );
    let _ = run(async { GitRunner::discover().await.unwrap().output(cmd).await });
    let trusted = std::fs::read_to_string(&seen).unwrap();
    assert!(
        trusted.lines().any(|l| l == "[h.test]:22 SHA256:abc+/9"),
        "{trusted}"
    );
}
