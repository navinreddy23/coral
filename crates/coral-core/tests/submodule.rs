//! Submodules, read from `.gitmodules` and the index rather than from `git submodule status`.

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::submodule::{parse_gitlinks, parse_gitmodules};
use coral_core::testutil::TestRepo;

#[test]
fn a_repository_without_gitmodules_has_no_submodules() {
    let repo = TestRepo::new().write("a.txt", "a").commit("first");
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        assert!(loc.submodules(&runner).await.unwrap().is_empty());
    });
}

#[test]
fn a_declared_submodule_is_listed_with_the_commit_the_index_pins() {
    let inner = TestRepo::new().write("lib.txt", "v1").commit("inner");
    let inner_head = inner.git(["rev-parse", "HEAD"]).trim().to_owned();

    let repo = TestRepo::new().write("a.txt", "a").commit("first");
    repo.git([
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        inner.path().to_str().unwrap(),
        "external/dev-scripts",
    ]);
    let repo = repo.commit("add submodule");

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let subs = loc.submodules(&runner).await.unwrap();

        assert_eq!(subs.len(), 1, "one submodule");
        let s = &subs[0];
        assert_eq!(s.path, "external/dev-scripts");
        assert_eq!(s.name, "external/dev-scripts");
        assert_eq!(s.pinned.as_deref(), Some(inner_head.as_str()));
        assert!(s.initialised, "`submodule add` clones it straight away");
    });
}

#[test]
fn a_path_with_a_space_survives_both_reads() {
    // The reason neither read goes through `git submodule status`: its ` (describe)` suffix
    // makes a path containing a space impossible to split off unambiguously.
    let config =
        b"submodule.my tools.path\nvendor/my tools\0submodule.my tools.url\nhttps://e.x/t\0";
    let subs = parse_gitmodules(config);
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].name, "my tools");
    assert_eq!(subs[0].path, "vendor/my tools");
    assert_eq!(subs[0].url, "https://e.x/t");

    let links = parse_gitlinks(b"160000 abc123 0\tvendor/my tools\x00100644 def456 0\ta.txt\x00");
    assert_eq!(
        links.get("vendor/my tools").map(String::as_str),
        Some("abc123")
    );
    assert_eq!(links.len(), 1, "only gitlinks, not ordinary blobs");
}

#[test]
fn a_subsection_name_containing_dots_is_kept_whole() {
    // `submodule.<name>.path` splits from the ends, not on the second dot: a name is free to
    // contain them and routinely does when it is taken from a hostname or a path.
    let subs = parse_gitmodules(b"submodule.a.b.c.path\nvendor/x\0submodule.a.b.c.url\ng://u\0");
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].name, "a.b.c");
    assert_eq!(subs[0].path, "vendor/x");
}

#[test]
fn a_declaration_without_a_path_is_dropped() {
    // Nothing to show in the sidebar and nowhere to open, so it is not a row.
    let subs = parse_gitmodules(b"submodule.orphan.url\nhttps://example.com/x\0");
    assert!(subs.is_empty());
}

/// A superproject with one submodule, cloned and committed.
fn with_submodule() -> (
    coral_core::testutil::TestRepo,
    coral_core::testutil::TestRepo,
) {
    let inner = TestRepo::new().write("lib.txt", "v1").commit("inner one");
    let repo = TestRepo::new().write("a.txt", "a").commit("first");
    repo.git([
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        inner.path().to_str().unwrap(),
        "external/dev-scripts",
    ]);
    let repo = repo.commit("add the submodule");
    (repo, inner)
}

fn run<F, T>(f: F) -> T
where
    F: std::future::Future<Output = T>,
{
    tokio::runtime::Runtime::new().unwrap().block_on(f)
}

async fn located(repo: &coral_core::testutil::TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

#[test]
fn the_recorded_revision_is_read_from_inside_the_submodule() {
    // The superproject records an object id and nothing else, so the message can only come
    // from the submodule's own object store.
    let (repo, _inner) = with_submodule();
    let revision = run(async {
        let (runner, loc) = located(&repo).await;
        loc.submodule_revision(&runner, "external/dev-scripts")
            .await
            .unwrap()
    });

    let found = revision.expect("the submodule has a working copy");
    assert_eq!(found.summary, "inner one");
    assert_eq!(found.oid.len(), 40);
    assert!(found.in_sync, "it sits where the superproject pins it");
}

#[test]
fn a_submodule_that_has_moved_is_reported_out_of_sync() {
    let (repo, inner) = with_submodule();
    // Commit inside the submodule without recording it in the superproject.
    let at = repo.path().join("external/dev-scripts");
    std::fs::write(at.join("lib.txt"), "v2").unwrap();
    for args in [
        vec!["add", "-A"],
        vec!["commit", "--quiet", "-m", "inner two"],
    ] {
        assert!(
            std::process::Command::new("git")
                .args(&args)
                .current_dir(&at)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_AUTHOR_NAME", "Coral Fixture")
                .env("GIT_AUTHOR_EMAIL", "fixture@coral.test")
                .env("GIT_COMMITTER_NAME", "Coral Fixture")
                .env("GIT_COMMITTER_EMAIL", "fixture@coral.test")
                .status()
                .unwrap()
                .success()
        );
    }
    let _ = &inner;

    let revision = run(async {
        let (runner, loc) = located(&repo).await;
        loc.submodule_revision(&runner, "external/dev-scripts")
            .await
            .unwrap()
    });
    let found = revision.expect("still has a working copy");
    assert_eq!(found.summary, "inner two");
    assert!(!found.in_sync, "the superproject still pins the old commit");
}

#[test]
fn a_submodule_with_no_working_copy_has_no_revision_to_report() {
    let (repo, _inner) = with_submodule();
    repo.git([
        "submodule",
        "deinit",
        "--force",
        "--",
        "external/dev-scripts",
    ]);

    let revision = run(async {
        let (runner, loc) = located(&repo).await;
        loc.submodule_revision(&runner, "external/dev-scripts")
            .await
            .unwrap()
    });
    assert!(revision.is_none(), "there is no object store to read");
}

#[test]
fn changing_the_url_writes_both_gitmodules_and_the_working_configuration() {
    // `set-url` alone commits the new URL but leaves the next fetch going to the old one;
    // `sync` is what copies it into the configuration git actually reads.
    let (repo, _inner) = with_submodule();
    run(async {
        let (runner, loc) = located(&repo).await;
        loc.submodule_set_url(
            &runner,
            "external/dev-scripts",
            "https://example.com/new.git",
        )
        .await
        .unwrap();
    });

    let declared = repo.git([
        "config",
        "--file",
        ".gitmodules",
        "--get",
        "submodule.external/dev-scripts.url",
    ]);
    assert_eq!(declared, "https://example.com/new.git");

    let effective = repo.git(["config", "--get", "submodule.external/dev-scripts.url"]);
    assert_eq!(
        effective, "https://example.com/new.git",
        "sync must copy it into the working configuration"
    );
}

#[test]
fn removing_a_submodule_takes_its_working_copy_its_entry_and_its_clone() {
    // All three, because leaving the clone behind is what makes adding it back at the same
    // path fail with "already exists in the index".
    let (repo, _inner) = with_submodule();
    let module = repo.path().join(".git/modules/external/dev-scripts");
    assert!(module.exists(), "the clone is there to begin with");

    run(async {
        let (runner, loc) = located(&repo).await;
        loc.submodule_remove(&runner, "external/dev-scripts", true)
            .await
            .unwrap();
    });

    assert!(!repo.path().join("external/dev-scripts").exists());
    assert!(!module.exists(), "the clone under .git/modules goes too");
    let listed = run(async {
        let (runner, loc) = located(&repo).await;
        loc.submodules(&runner).await.unwrap()
    });
    assert!(listed.is_empty(), "{listed:?}");
}

#[test]
fn an_update_to_the_branch_tip_is_a_different_operation_from_one_to_the_recorded_commit() {
    // Without --remote git checks out what the superproject records; with it, git fetches the
    // configured branch and moves to its tip, which changes what will be recorded next.
    let (repo, inner) = with_submodule();
    let pinned = run(async {
        let (runner, loc) = located(&repo).await;
        loc.submodules(&runner).await.unwrap()[0].pinned.clone()
    });

    // git refuses the `file` transport for submodules by default — CVE-2022-39253 — and Coral
    // does not override that: a user who wants a local submodule URL allows it themselves. It
    // has to be set inside the submodule, since that is where the fetch actually runs; setting
    // it on the superproject alone leaves the fetch refusing.
    assert!(
        std::process::Command::new("git")
            .args(["config", "protocol.file.allow", "always"])
            .current_dir(repo.path().join("external/dev-scripts"))
            .status()
            .unwrap()
            .success()
    );

    // A new commit on the submodule's own default branch.
    let inner = inner.write("lib.txt", "v2").commit("inner two");
    let tip = inner.git(["rev-parse", "HEAD"]);
    let branch = inner.git(["branch", "--show-current"]);
    repo.git([
        "config",
        "--file",
        ".gitmodules",
        "submodule.external/dev-scripts.branch",
        &branch,
    ]);

    run(async {
        let (runner, loc) = located(&repo).await;
        loc.submodule_init(&runner, Some("external/dev-scripts"), false, false)
            .await
            .unwrap();
        let at = loc
            .submodule_revision(&runner, "external/dev-scripts")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            Some(at.oid.clone()),
            pinned,
            "without --remote it stays on the recorded commit"
        );

        loc.submodule_init(&runner, Some("external/dev-scripts"), false, true)
            .await
            .unwrap();
        let moved = loc
            .submodule_revision(&runner, "external/dev-scripts")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(moved.oid, tip, "with --remote it moves to the branch tip");
        assert!(!moved.in_sync, "and the superproject has not recorded that");
    });
}
