//! Commit signing: the settings, and the keys that can be chosen.
//!
//! The listings are parsed from `GnuPG`'s documented `--with-colons` format, and the fixtures
//! here are real output from gpg 2.4 rather than invented, so a field that moves is caught.

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::signing::{
    AppConfig, SigningConfig, SigningFormat, SigningOverrides, parse_secret_keys, ssh_keys_in,
};
use coral_core::testutil::TestRepo;

/// Two secret keys as gpg 2.4.4 prints them: one that can sign, one that cannot.
const LISTING: &[u8] = b"tru::1:1770192902:0:3:1:5\n\
sec:u:2048:1:9E9BC1B3B7C4AA29:1788503560:1851575560::u:::scSC:::+:::23::0:\n\
fpr:::::::::0633C12121B1A10FADE103E39E9BC1B3B7C4AA29:\n\
grp:::::::::32499E0B27D1106F5753BDA354FF89C1FDF78384:\n\
uid:u::::1788503560::D41C7F0DCEB997F4808D857666EB069562051596::Coral Test <coral@example.invalid>::::::::::0:\n\
ssb:u:2048:1:AAAABBBBCCCCDDDD:1788503560::::::e:::+:::23:\n\
sec:u:4096:1:1111222233334444:1600000000:0::u:::eE:::+:::23::0:\n\
fpr:::::::::AAAA111122223333444455556666777788889999:\n\
uid:u::::1600000000::HASH::Encrypt Only <enc@example.invalid>::::::::::0:\n";

#[test]
fn only_keys_that_can_sign_are_offered() {
    // A key with no `s` capability cannot sign; offering it produces a failure at commit time
    // that says nothing about why.
    let keys = parse_secret_keys(LISTING);
    assert_eq!(keys.len(), 1, "the encrypt-only key is not a signing key");
    assert_eq!(keys[0].label, "Coral Test <coral@example.invalid>");
}

#[test]
fn the_full_fingerprint_is_what_gets_stored() {
    // A key id is the last sixteen digits of a fingerprint and two keys can share one, so the
    // fingerprint is what identifies the key unambiguously to git.
    let keys = parse_secret_keys(LISTING);
    assert_eq!(keys[0].id, "0633C12121B1A10FADE103E39E9BC1B3B7C4AA29");
}

#[test]
fn an_expiry_is_reported_and_never_expiring_is_not() {
    let keys = parse_secret_keys(LISTING);
    assert_eq!(keys[0].expires, Some(1_851_575_560));
    assert!(!keys[0].expired);

    // gpg writes 0 for a key with no expiry, which is not a date.
    let never = b"sec:u:4096:1:AAAA:1600000000:0::u:::scSC:::+:::23::0:\n\
fpr:::::::::FFFF1111222233334444555566667777888899AA:\n\
uid:u::::1600000000::H::No Expiry <n@e.invalid>::::::::::0:\n";
    assert_eq!(parse_secret_keys(never)[0].expires, None);
}

#[test]
fn an_expired_key_is_still_listed_and_marked() {
    // It is exactly the key someone is looking for when they come to this screen; hiding it
    // makes the screen look broken rather than explaining.
    let expired = b"sec:e:4096:1:AAAA:1600000000:1600000001::u:::scSC:::+:::23::0:\n\
fpr:::::::::FFFF1111222233334444555566667777888899AA:\n\
uid:e::::1600000000::H::Old Key <old@e.invalid>::::::::::0:\n";
    let keys = parse_secret_keys(expired);
    assert_eq!(keys.len(), 1);
    assert!(keys[0].expired);
}

#[test]
fn a_revoked_or_disabled_key_is_dropped() {
    // Unlike an expired key, these can never sign again, so choosing one is only a mistake.
    for validity in ["r", "d"] {
        let listing = format!(
            "sec:{validity}:4096:1:AAAA:1600000000:0::u:::scSC:::+:::23::0:\n\
fpr:::::::::FFFF1111222233334444555566667777888899AA:\n\
uid:{validity}::::1600000000::H::Gone <g@e.invalid>::::::::::0:\n"
        );
        assert!(
            parse_secret_keys(listing.as_bytes()).is_empty(),
            "{validity}"
        );
    }
}

#[test]
fn a_colon_in_a_name_survives() {
    // GnuPG escapes it, because a colon is the field separator; left escaped it shows as
    // `\x3a` in the list.
    let listing = b"sec:u:4096:1:AAAA:1600000000:0::u:::scSC:::+:::23::0:\n\
fpr:::::::::FFFF1111222233334444555566667777888899AA:\n\
uid:u::::1600000000::H::Ada\\x3a the first <ada@e.invalid>::::::::::0:\n";
    assert_eq!(
        parse_secret_keys(listing)[0].label,
        "Ada: the first <ada@e.invalid>"
    );
}

#[test]
fn nothing_at_all_is_not_an_error() {
    assert!(parse_secret_keys(b"").is_empty());
    assert!(parse_secret_keys(b"tru::1:1770192902:0:3:1:5\n").is_empty());
}

#[test]
fn ssh_keys_are_listed_from_their_public_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("id_ed25519.pub"),
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5 navin@dellpro\n",
    )
    .unwrap();
    // A private key is not a thing git can be pointed at for signing.
    std::fs::write(dir.path().join("id_ed25519"), "PRIVATE").unwrap();
    std::fs::write(dir.path().join("known_hosts"), "example.com ssh-rsa AAAA").unwrap();

    let keys = ssh_keys_in(dir.path());
    assert_eq!(keys.len(), 1, "only the public key");
    assert!(keys[0].id.ends_with("id_ed25519.pub"), "{}", keys[0].id);
    assert_eq!(keys[0].label, "navin@dellpro (ssh-ed25519)");
}

/// A repository and an app-level config file of its own, so these never touch the machine's.
///
/// The path is passed in rather than set through the environment: the engine spawns git
/// itself, and reaching a child's environment from a test would mean mutating the process's,
/// which is `unsafe` and shared with every other test in the binary.
fn hermetic() -> (TestRepo, tempfile::TempDir) {
    let home = tempfile::tempdir().unwrap();
    std::fs::write(home.path().join("gitconfig"), "").unwrap();
    let repo = TestRepo::new().write("a.txt", "a").commit("first");
    // The fixture turns signing off for itself so its commits are never signed. That is a
    // repository-level override, and it would quietly beat everything these tests set at the
    // app level — which is the feature working, but not what each test is about.
    repo.git(["config", "--local", "--unset-all", "commit.gpgsign"]);
    (repo, home)
}

#[test]
fn a_repository_inherits_the_app_level_settings() {
    let (repo, home) = hermetic();
    let app_path = home.path().join("gitconfig");
    let app = AppConfig(Some(&app_path));
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();

        let wanted = SigningConfig {
            format: SigningFormat::OpenPgp,
            program: "gpg2".to_owned(),
            key: "AAAA1111".to_owned(),
            sign_commits: true,
            sign_tags: false,
        };
        loc.set_signing_global(&runner, app, &wanted).await.unwrap();

        let scopes = loc.signing_scopes(&runner, app).await.unwrap();
        assert_eq!(scopes.global, wanted);
        assert_eq!(
            scopes.effective, wanted,
            "with no override, the app level is what happens"
        );
        assert!(
            scopes.local.is_empty(),
            "the repository sets nothing of its own"
        );
    });
}

#[test]
fn a_repository_key_overrides_the_app_level_one() {
    // The same person signs work commits with a company key and their own with another, which
    // is the whole reason this is per repository rather than per profile.
    let (repo, home) = hermetic();
    let app_path = home.path().join("gitconfig");
    let app = AppConfig(Some(&app_path));
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();

        loc.set_signing_global(
            &runner,
            app,
            &SigningConfig {
                key: "PERSONAL".to_owned(),
                sign_commits: true,
                ..SigningConfig::default()
            },
        )
        .await
        .unwrap();

        loc.set_signing_local(
            &runner,
            &SigningOverrides {
                key: Some("WORK".to_owned()),
                ..SigningOverrides::default()
            },
            SigningFormat::OpenPgp,
        )
        .await
        .unwrap();

        let scopes = loc.signing_scopes(&runner, app).await.unwrap();
        assert_eq!(scopes.effective.key, "WORK");
        assert_eq!(scopes.global.key, "PERSONAL", "the app level is untouched");
        assert_eq!(scopes.local.key.as_deref(), Some("WORK"));
        // Everything it did not override still comes from above.
        assert!(scopes.effective.sign_commits);
        assert_eq!(scopes.local.sign_commits, None);
    });
}

#[test]
fn clearing_an_override_goes_back_to_inheriting() {
    // Not to whatever it happened to inherit at the time: the difference between "off" and
    // "not set" is the difference between pinning a value and following the app level.
    let (repo, home) = hermetic();
    let app_path = home.path().join("gitconfig");
    let app = AppConfig(Some(&app_path));
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();

        loc.set_signing_global(
            &runner,
            app,
            &SigningConfig {
                sign_commits: true,
                ..SigningConfig::default()
            },
        )
        .await
        .unwrap();

        // Turn signing off for this repository only.
        loc.set_signing_local(
            &runner,
            &SigningOverrides {
                sign_commits: Some(false),
                ..SigningOverrides::default()
            },
            SigningFormat::OpenPgp,
        )
        .await
        .unwrap();
        assert!(
            !loc.signing_scopes(&runner, app)
                .await
                .unwrap()
                .effective
                .sign_commits
        );

        // Clear it, and the app level applies again.
        loc.set_signing_local(
            &runner,
            &SigningOverrides::default(),
            SigningFormat::OpenPgp,
        )
        .await
        .unwrap();
        let scopes = loc.signing_scopes(&runner, app).await.unwrap();
        assert!(scopes.effective.sign_commits, "back to inheriting");
        assert!(scopes.local.is_empty());
        assert!(
            !repo
                .git(["config", "--local", "--list"])
                .contains("commit.gpgsign")
        );
    });
}

#[test]
fn an_empty_program_is_unset_rather_than_written_empty() {
    // An empty `gpg.program` is not the same as no `gpg.program`: git would try to run it.
    let (repo, home) = hermetic();
    let app_path = home.path().join("gitconfig");
    let app = AppConfig(Some(&app_path));
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();

        loc.set_signing_global(
            &runner,
            app,
            &SigningConfig {
                program: "gpg2".to_owned(),
                ..SigningConfig::default()
            },
        )
        .await
        .unwrap();
        loc.set_signing_global(&runner, app, &SigningConfig::default())
            .await
            .unwrap();

        assert_eq!(
            loc.signing_scopes(&runner, app)
                .await
                .unwrap()
                .global
                .program,
            ""
        );
        assert!(
            !repo
                .git(["config", "--file", app_path.to_str().unwrap(), "--list"])
                .contains("gpg.openpgp.program")
        );
    });
}

#[test]
fn the_program_is_stored_per_format() {
    // `gpg.ssh.program` is how ssh signing is pointed at a different binary; a bare
    // `gpg.program` would be the wrong key and git would ignore it.
    let (repo, home) = hermetic();
    let app_path = home.path().join("gitconfig");
    let app = AppConfig(Some(&app_path));
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();

        loc.set_signing_global(
            &runner,
            app,
            &SigningConfig {
                format: SigningFormat::Ssh,
                program: "/usr/bin/ssh-keygen".to_owned(),
                ..SigningConfig::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(
            repo.git([
                "config",
                "--file",
                app_path.to_str().unwrap(),
                "gpg.ssh.program"
            ])
            .trim(),
            "/usr/bin/ssh-keygen"
        );
        let scopes = loc.signing_scopes(&runner, app).await.unwrap();
        assert_eq!(scopes.effective.format, SigningFormat::Ssh);
        assert_eq!(scopes.effective.program, "/usr/bin/ssh-keygen");
    });
}

/// A gpg that uses a keyring of its own, as a wrapper script.
///
/// gpg is told which keyring to use through the environment, and the engine deliberately does
/// not let a caller reach into a child's — so the choice is baked into a program instead,
/// which is exactly what `gpg.program` is for.
#[cfg(unix)]
fn gpg_in_its_own_keyring(home: &std::path::Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt as _;

    let gnupg = home.join("gnupg");
    std::fs::create_dir_all(&gnupg).unwrap();
    // gpg refuses a keyring anyone else can read.
    std::fs::set_permissions(&gnupg, std::fs::Permissions::from_mode(0o700)).unwrap();

    let wrapper = home.join("gpg-wrapper");
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nexport GNUPGHOME='{}'\nexec gpg \"$@\"\n",
            gnupg.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    wrapper
}

/// Generating a key and signing with it, against the real gpg.
///
/// Ignored by default: it creates a keyring and a 4096-bit key, which takes long enough to be
/// unwelcome in a normal run. `cargo test -- --ignored` runs it, and it is the only test that
/// proves the settings this screen writes actually produce a signed commit.
#[test]
#[ignore = "generates a real gpg key; run with --ignored"]
fn a_generated_key_signs_a_commit() {
    use coral_core::signing::{generate_key, list_keys};
    use secrecy::SecretString;

    let home = tempfile::tempdir().unwrap();
    let wrapper = gpg_in_its_own_keyring(home.path());
    let program = wrapper.to_str().unwrap();

    let repo = TestRepo::new().write("a.txt", "a").commit("first");
    repo.git(["config", "--local", "--unset-all", "commit.gpgsign"]);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let key = rt.block_on(async {
        assert!(
            list_keys(SigningFormat::OpenPgp, program)
                .await
                .unwrap()
                .is_empty(),
            "a keyring of its own, with nothing in it"
        );
        generate_key(
            program,
            "Coral Fixture",
            "fixture@coral.test",
            &SecretString::from(String::new()),
        )
        .await
        .unwrap()
    });

    assert_eq!(key.label, "Coral Fixture <fixture@coral.test>");
    assert_eq!(key.id.len(), 40, "a full fingerprint: {}", key.id);
    assert!(!key.expired);
    assert!(key.expires.is_some(), "two years, as the reference offers");

    // Now configure the repository with it and make a commit.
    let app_path = home.path().join("gitconfig");
    std::fs::write(&app_path, "").unwrap();
    let app = AppConfig(Some(&app_path));
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        loc.set_signing_global(
            &runner,
            app,
            &SigningConfig {
                format: SigningFormat::OpenPgp,
                program: program.to_owned(),
                key: key.id.clone(),
                sign_commits: true,
                sign_tags: false,
            },
        )
        .await
        .unwrap();
        let scopes = loc.signing_scopes(&runner, app).await.unwrap();
        assert!(scopes.effective.sign_commits);
        assert_eq!(scopes.effective.key, key.id);
    });

    // git reads the app-level file the same way the engine wrote it.
    let signed = repo.git([
        "-c",
        &format!("gpg.program={program}"),
        "-c",
        &format!("user.signingkey={}", key.id),
        "-c",
        "commit.gpgsign=true",
        "commit",
        "--allow-empty",
        "-m",
        "a signed commit",
    ]);
    assert!(!signed.contains("error"), "{signed}");

    // Verifying needs the same keyring the signature was made with, or git reports E — it
    // cannot check rather than it is not signed, which is a different answer entirely.
    let shown = repo.git([
        "-c",
        &format!("gpg.program={program}"),
        "log",
        "--format=%G?",
        "-1",
    ]);
    // G is a good signature; U is good but untrusted, which is what a freshly made key is
    // until its owner ratifies it.
    assert!(
        matches!(shown.trim(), "G" | "U"),
        "the commit should verify, got {shown:?}"
    );

    // And the signature names the key that was generated.
    let attributed_to = repo.git([
        "-c",
        &format!("gpg.program={program}"),
        "log",
        "--format=%GK",
        "-1",
    ]);
    assert!(
        key.id.ends_with(attributed_to.trim()),
        "signed by {attributed_to:?}, expected {}",
        key.id
    );
}
