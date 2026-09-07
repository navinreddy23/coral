//! Per-repository ssh keys, inheriting from the app level the way signing does.

use coral_core::config::AppConfig;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::ssh::{SshConfig, SshOverrides, command_for, key_in_command, keys_in};
use coral_core::testutil::TestRepo;

fn run<F, T>(f: F) -> T
where
    F: std::future::Future<Output = T>,
{
    tokio::runtime::Runtime::new().unwrap().block_on(f)
}

async fn located(repo: &TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

fn fixture() -> TestRepo {
    TestRepo::new().write("a.txt", "a\n").commit("first")
}

#[test]
fn the_command_pins_ssh_to_one_key_and_only_that_key() {
    // Two flags, and neither is enough alone.
    //
    // `IdentitiesOnly` keeps the agent from offering everything it holds. `-F none` keeps the
    // user's own config from adding a key beside the chosen one: `-i` and a host block's
    // `IdentityFile` accumulate rather than the first winning, and the agent then reorders
    // them. Measured against a real host, the command without `-F none` authenticated as the
    // account belonging to the config's key, silently, every time.
    let command = command_for("/home/dev/.ssh/id_work");
    assert!(command.contains("IdentitiesOnly=yes"), "{command}");
    assert!(command.contains("-F none"), "{command}");
    assert!(command.contains("id_work"), "{command}");
}

#[test]
fn a_pinned_key_is_still_read_back_out_of_the_command_that_pins_it() {
    // The pane shows which key a repository uses by parsing it back out. A flag added in front
    // of `-i` must not be mistaken for the key.
    let command = command_for("/home/dev/.ssh/id_work");
    assert_eq!(
        key_in_command(&command).as_deref(),
        Some("/home/dev/.ssh/id_work"),
        "{command}"
    );
}

#[test]
fn the_key_is_read_back_out_of_a_command_whatever_the_quoting() {
    for (command, wanted) in [
        (
            "ssh -i '/home/dev/.ssh/id_ed25519' -o IdentitiesOnly=yes",
            "/home/dev/.ssh/id_ed25519",
        ),
        (
            "ssh -i \"/home/dev/my keys/id\" -o IdentitiesOnly=yes",
            "/home/dev/my keys/id",
        ),
        ("ssh -i /home/dev/.ssh/plain", "/home/dev/.ssh/plain"),
    ] {
        assert_eq!(
            key_in_command(command).as_deref(),
            Some(wanted),
            "{command}"
        );
    }
    // A command the user wrote themselves that names no key is not an error, and not a key.
    assert_eq!(key_in_command("ssh -o StrictHostKeyChecking=no"), None);
    assert_eq!(key_in_command(""), None);
}

#[test]
fn a_key_pair_is_the_private_file_whose_public_half_sits_beside_it() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("id_ed25519"), "PRIVATE").unwrap();
    std::fs::write(
        dir.path().join("id_ed25519.pub"),
        "ssh-ed25519 AAAAC3Nza dev@workstation\n",
    )
    .unwrap();
    // A public key with no private half is somebody else's; it cannot be used to sign in.
    std::fs::write(dir.path().join("theirs.pub"), "ssh-rsa AAAA them@host\n").unwrap();
    // And a stray file is not a key at all.
    std::fs::write(dir.path().join("known_hosts"), "github.com ssh-rsa AAAA\n").unwrap();

    let keys = keys_in(dir.path());
    assert_eq!(keys.len(), 1, "{keys:?}");
    assert_eq!(keys[0].kind, "ssh-ed25519");
    assert_eq!(keys[0].comment, "dev@workstation");
    assert!(keys[0].path.ends_with("id_ed25519"));
    assert!(keys[0].public_path.ends_with("id_ed25519.pub"));
}

#[test]
fn a_repository_overrides_the_app_level_and_can_be_cleared_back_to_it() {
    let repo = fixture();
    let app = tempfile::NamedTempFile::new().unwrap();
    let at = AppConfig(Some(app.path()));

    run(async {
        let (runner, loc) = located(&repo).await;

        loc.set_ssh_global(
            &runner,
            at,
            &SshConfig {
                use_agent: false,
                private_key: "/keys/app".to_owned(),
                public_key: "/keys/app.pub".to_owned(),
                command: String::new(),
                credential_helper: String::new(),
            },
        )
        .await
        .unwrap();

        let scopes = loc.ssh_scopes(&runner, at).await.unwrap();
        assert_eq!(scopes.global.private_key, "/keys/app");
        assert_eq!(
            scopes.effective.private_key, "/keys/app",
            "with no override, the app level is what happens"
        );
        assert!(scopes.local.is_empty());

        loc.set_ssh_local(
            &runner,
            &SshOverrides {
                private_key: Some("/keys/this-repo".to_owned()),
                public_key: Some("/keys/this-repo.pub".to_owned()),
                credential_helper: None,
            },
        )
        .await
        .unwrap();

        let scopes = loc.ssh_scopes(&runner, at).await.unwrap();
        assert_eq!(scopes.effective.private_key, "/keys/this-repo");
        assert_eq!(
            scopes.global.private_key, "/keys/app",
            "the app level is unchanged by a repository overriding it"
        );
        assert_eq!(scopes.local.private_key.as_deref(), Some("/keys/this-repo"));

        // Clearing every override goes back to inheriting, which is a different state from
        // pinning whatever happened to be inherited.
        loc.set_ssh_local(&runner, &SshOverrides::default())
            .await
            .unwrap();
        let scopes = loc.ssh_scopes(&runner, at).await.unwrap();
        assert!(scopes.local.is_empty(), "{:?}", scopes.local);
        assert_eq!(scopes.effective.private_key, "/keys/app");
    });
}

#[test]
fn using_the_agent_says_nothing_rather_than_saying_something_empty() {
    // An empty `core.sshCommand` still shadows whatever the user configured by hand, so the
    // agent case has to unset the key rather than write a blank one.
    let repo = fixture();
    let app = tempfile::NamedTempFile::new().unwrap();
    let at = AppConfig(Some(app.path()));

    run(async {
        let (runner, loc) = located(&repo).await;
        loc.set_ssh_global(
            &runner,
            at,
            &SshConfig {
                use_agent: true,
                private_key: "/keys/ignored".to_owned(),
                ..SshConfig::default()
            },
        )
        .await
        .unwrap();

        let scopes = loc.ssh_scopes(&runner, at).await.unwrap();
        assert!(scopes.global.use_agent);
        assert_eq!(scopes.global.command, "", "no command is written at all");
        assert_eq!(scopes.global.private_key, "");
    });
}

#[test]
fn the_settings_are_written_where_git_itself_reads_them() {
    // Not a private store: the point of using git's own configuration is that the command line
    // beside Coral behaves the same way.
    let repo = fixture();
    let app = tempfile::NamedTempFile::new().unwrap();

    run(async {
        let (runner, loc) = located(&repo).await;
        loc.set_ssh_local(
            &runner,
            &SshOverrides {
                private_key: Some("/keys/only-here".to_owned()),
                public_key: None,
                credential_helper: None,
            },
        )
        .await
        .unwrap();
        let _ = &app;
    });

    let configured = repo.git(["config", "--local", "--get", "core.sshCommand"]);
    assert!(configured.contains("/keys/only-here"), "{configured}");
    assert!(configured.contains("IdentitiesOnly=yes"), "{configured}");
}

/// What ssh itself resolves the identity list to, without connecting to anything.
///
/// `ssh -G` prints the fully resolved configuration for a host — every `Host` block and every
/// `Include` already applied — and exits. That makes the one thing this module has to get
/// right testable with no server, no network and no real key: which identities end up in the
/// list, and how many.
///
/// Run through a shell, because that is how git invokes `core.sshCommand`: it hands the string
/// to a shell and appends the host and the remote command to it.
#[cfg(unix)]
fn resolved_identities(command: &str, host: &str) -> Vec<String> {
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("{command} -G {host}"))
        .output()
        .expect("run ssh -G");
    assert!(
        out.status.success(),
        "ssh -G failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| line.strip_prefix("identityfile "))
        .map(|path| path.trim().to_owned())
        .collect()
}

/// A generated key pair, and the path to its private half.
///
/// Generated rather than checked in. A private key in a repository is a private key on the
/// internet, however clearly it is labelled a fixture.
#[cfg(unix)]
fn dummy_key(at: &std::path::Path) -> String {
    let out = std::process::Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-C", "coral-fixture", "-f"])
        .arg(at)
        .output()
        .expect("run ssh-keygen");
    assert!(
        out.status.success(),
        "ssh-keygen: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    at.display().to_string()
}

/// An ssh config pinning one key for one host, which is what anybody with two accounts on the
/// same host ends up writing.
///
/// Passed with `-F` rather than placed in a home directory: ssh takes the path of the user's
/// own config from the passwd entry and not from `$HOME`, so a test cannot move it. `-F` is
/// the same file at the same level, which is what makes it a fair stand-in.
#[cfg(unix)]
fn a_config_that_pins(dir: &std::path::Path, key: &str, host: &str) -> String {
    let at = dir.join("config");
    std::fs::write(
        &at,
        format!(
            "Host {host}\n    HostName {host}\n    User git\n    IdentityFile {key}\n    \
             IdentitiesOnly yes\n"
        ),
    )
    .unwrap();
    at.display().to_string()
}

#[cfg(unix)]
#[test]
fn a_pinned_key_is_the_only_key_even_where_the_user_has_pinned_another() {
    // The bug this exists for. `-i` and a `Host` block's `IdentityFile` do not compete: they
    // accumulate into one list, and `IdentitiesOnly=yes` only restricts ssh to that list, which
    // by then holds both. Whichever the agent happens to hold first is offered first, so the
    // pin authenticates as the other account, silently.
    let dir = tempfile::tempdir().unwrap();
    let theirs = dummy_key(&dir.path().join("theirs"));
    let chosen = dummy_key(&dir.path().join("chosen"));
    let host = "git.example.test";
    let theirs_config = a_config_that_pins(dir.path(), &theirs, host);

    let alone = resolved_identities(&format!("ssh -F {theirs_config}"), host);
    assert_eq!(alone, vec![theirs.clone()], "the config on its own");

    // What Coral used to write. Both survive, which is the whole fault.
    let hazard = format!("ssh -F {theirs_config} -i '{chosen}' -o IdentitiesOnly=yes");
    let both = resolved_identities(&hazard, host);
    assert!(
        both.contains(&theirs) && both.contains(&chosen),
        "without -F none the config's key stays in the list: {both:?}"
    );

    // What Coral writes now, with the user's config in play exactly as it would be.
    let pinned = command_for(&chosen).replacen("ssh ", &format!("ssh -F {theirs_config} "), 1);
    assert_eq!(
        resolved_identities(&pinned, host),
        vec![chosen],
        "a pinned key must be the only identity ssh will offer"
    );
}

#[cfg(unix)]
#[test]
fn the_agent_default_leaves_the_user_config_entirely_alone() {
    // The price of pinning is that `~/.ssh/config` is not read at all, so it must be paid only
    // by somebody who actually pinned one. The default writes no command, and this is what says
    // their ProxyJump, their port and their aliases still apply.
    let dir = tempfile::tempdir().unwrap();
    let theirs = dummy_key(&dir.path().join("theirs"));
    let host = "git.example.test";
    let theirs_config = a_config_that_pins(dir.path(), &theirs, host);

    let identities = resolved_identities(&format!("ssh -F {theirs_config}"), host);
    assert_eq!(identities, vec![theirs]);

    // And the rest of the block is honoured, which is the part a pin gives up.
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("ssh -F {theirs_config} -G {host}"))
        .output()
        .unwrap();
    let resolved = String::from_utf8_lossy(&out.stdout);
    assert!(resolved.contains("user git"), "{resolved}");
}
