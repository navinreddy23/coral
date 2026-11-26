//! `coral submodule-add` and `coral submodule-init`.
//!
//! A successful add clones from a local path, which git refuses to a submodule unless
//! `protocol.file.allow` says otherwise (CVE-2022-39253). The clone runs in the submodule's own
//! configuration, so only the global level reaches it, and a test cannot change its own
//! environment without `unsafe`. Those tests run the binary instead, with a global
//! configuration of their own.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use coral_core::testutil::TestRepo;

fn argv(args: &[&str]) -> Vec<OsString> {
    std::iter::once("coral")
        .chain(args.iter().copied())
        .map(OsString::from)
        .collect()
}

/// Runs the binary against `repo` with the file transport allowed, and returns its envelope.
fn coral(repo: &Path, args: &[&str]) -> serde_json::Value {
    let home = tempfile::tempdir().unwrap();
    let config = home.path().join("gitconfig");
    std::fs::write(&config, "[protocol \"file\"]\n\tallow = always\n").unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_coral"))
        .args(["--json", "--repo"])
        .arg(repo)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", &config)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path())
        .output()
        .unwrap();
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!("{e}: {}", String::from_utf8_lossy(&out.stderr));
    })
}

/// A repository to add, with a `dev` branch and a submodule of its own, published as `lib.git`
/// so the path git derives from its URL is `lib`. Every repository it names has to outlive the
/// test, so all of them are held here.
struct Published {
    _deep: TestRepo,
    _inner: TestRepo,
    _host: tempfile::TempDir,
    url: PathBuf,
}

impl Published {
    fn new() -> Self {
        let deep = TestRepo::new().write("deep.txt", "deep").commit("deep");
        let inner = TestRepo::new().write("lib.txt", "v1").commit("lib");
        inner.git([
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            deep.path().to_str().unwrap(),
            "deep",
        ]);
        let inner = inner.commit("nest deep");
        inner.git(["branch", "dev"]);
        let inner = inner.write("lib.txt", "v2").commit("lib two");

        let host = tempfile::tempdir().unwrap();
        let url = host.path().join("lib.git");
        inner.git(["clone", "--quiet", "--bare", ".", url.to_str().unwrap()]);
        Self {
            _deep: deep,
            _inner: inner,
            _host: host,
            url,
        }
    }

    fn url(&self) -> &str {
        self.url.to_str().unwrap()
    }
}

#[test]
fn an_added_submodule_is_cloned_staged_and_its_own_submodules_cloned_too() {
    let lib = Published::new();
    let repo = TestRepo::new().write("a.txt", "a").commit("first");

    let out = coral(repo.path(), &["submodule-add", lib.url()]);
    assert_eq!(out["ok"], true, "{out}");
    let added = &out["result"]["submodule"];
    assert_eq!(
        added["path"], "lib",
        "derived from the URL as git derives it"
    );
    assert_eq!(added["initialised"], true);

    let staged = repo.git(["diff", "--cached", "--name-only"]);
    assert_eq!(staged.lines().collect::<Vec<_>>(), [".gitmodules", "lib"]);
    assert!(
        repo.git(["ls-files", "--stage", "lib"])
            .starts_with("160000 ")
    );
    assert!(
        repo.path().join("lib/deep/.git").exists(),
        "the submodule inside it is cloned as well"
    );
}

#[test]
fn a_branch_is_recorded_and_stays_checked_out() {
    let lib = Published::new();
    let repo = TestRepo::new().write("a.txt", "a").commit("first");

    let out = coral(
        repo.path(),
        &["submodule-add", lib.url(), "vendor/lib", "--branch", "dev"],
    );
    assert_eq!(out["ok"], true, "{out}");
    assert_eq!(
        repo.git(["config", "-f", ".gitmodules", "submodule.vendor/lib.branch"]),
        "dev"
    );
    let at = repo.path().join("vendor/lib");
    let branch = std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(&at)
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&branch.stdout).trim(),
        "dev",
        "the update after the add must not detach it"
    );
}

#[test]
fn a_key_of_its_own_is_kept_for_it_and_written_into_its_clone() {
    let lib = Published::new();
    let repo = TestRepo::new().write("a.txt", "a").commit("first");
    let key = "/home/dev/.ssh/id_work";

    let out = coral(repo.path(), &["submodule-add", lib.url(), "--ssh-key", key]);
    assert_eq!(out["ok"], true, "{out}");
    assert_eq!(repo.git(["config", "coral.submodule.lib.sshKey"]), key);

    let inside = std::process::Command::new("git")
        .args(["config", "--local", "core.sshCommand"])
        .current_dir(repo.path().join("lib"))
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&inside.stdout).trim(),
        coral_core::ssh::command_for(key),
        "a fetch from inside the submodule must find the same key"
    );
}

#[tokio::test]
async fn a_url_that_cannot_be_reached_is_reported_as_gits_error() {
    let repo = TestRepo::new().write("a.txt", "a").commit("first");
    let missing = repo.path().join("nowhere.git");
    let out = coral_cli::run(argv(&[
        "--json",
        "--repo",
        repo.path().to_str().unwrap(),
        "submodule-add",
        missing.to_str().unwrap(),
    ]))
    .await;
    assert_eq!(out.json["ok"], false, "{}", out.json);
    assert_eq!(out.json["error"]["code"], "git_error");
    assert!(!repo.path().join(".gitmodules").exists());
}

#[tokio::test]
async fn init_brings_back_a_working_copy_and_the_one_inside_it() {
    let lib = Published::new();
    let repo = TestRepo::new().write("a.txt", "a").commit("first");
    repo.git([
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        lib.url(),
        "lib",
    ]);
    repo.git([
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "update",
        "--init",
        "--recursive",
    ]);
    let repo = repo.commit("add lib");
    // The clones stay under .git/modules, so bringing the working copies back reaches nothing.
    repo.git(["submodule", "deinit", "--force", "--all"]);
    assert!(!repo.path().join("lib/.git").exists());

    let out = coral_cli::run(argv(&[
        "--json",
        "--repo",
        repo.path().to_str().unwrap(),
        "submodule-init",
    ]))
    .await;
    assert_eq!(out.json["ok"], true, "{}", out.json);
    assert_eq!(out.json["result"]["what"], "initialised every submodule");
    assert!(repo.path().join("lib/.git").exists());
    assert!(
        repo.path().join("lib/deep/.git").exists(),
        "nested submodules are initialised too"
    );
}

#[test]
fn a_submodule_added_where_gitmodules_still_names_one_is_added() {
    // Its gitlink removed and its .gitmodules entry left: git adds over the entry, and Coral
    // looked only for a path that had not been declared, said git had declared nothing new,
    // and left the nested submodule uncloned.
    let lib = Published::new();
    let repo = TestRepo::new().write("a.txt", "a").commit("first");
    for (key, value) in [
        ("submodule.lib.path", "lib"),
        ("submodule.lib.url", "https://example.invalid/old.git"),
    ] {
        repo.git(["config", "-f", ".gitmodules", key, value]);
    }
    let repo = repo.commit("a submodule removed by hand");

    let out = coral(repo.path(), &["submodule-add", lib.url(), "lib"]);
    assert_eq!(out["ok"], true, "{out}");
    assert_eq!(out["result"]["submodule"]["path"], "lib");
    assert!(
        repo.path().join("lib/deep/deep.txt").exists(),
        "with its own submodule"
    );
}
