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
    // `IdentitiesOnly` is the whole point. Without it ssh offers every key the agent holds
    // before the one it was told to use, and a server that accepts one of those authenticates
    // as the wrong account — which is the failure a per-repository key exists to prevent.
    let command = command_for("/home/dev/.ssh/id_work");
    assert!(command.contains("IdentitiesOnly=yes"), "{command}");
    assert!(command.contains("id_work"), "{command}");
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
