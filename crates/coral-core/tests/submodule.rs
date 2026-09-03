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
