use std::ffi::OsString;

use coral_core::testutil::TestRepo;

fn argv(args: &[&str]) -> Vec<OsString> {
    std::iter::once("coral")
        .chain(args.iter().copied())
        .map(OsString::from)
        .collect()
}

/// Machine- and version-specific fields, replaced so snapshots are stable everywhere.
macro_rules! stable {
    () => {{
        let mut r = insta::Settings::clone_current();
        r.add_redaction(".result.path", "[path]");
        r.add_redaction(".result.gitDir", "[gitdir]");
        r.add_redaction(".result.commonDir", "[commondir]");
        r.add_redaction(".result.gitVersion", "[gitversion]");
        r.add_redaction(".result.git", "[gitversion]");
        // The number itself is asserted directly where it matters. Pinning it here would make
        // every release edit a snapshot, which teaches everyone to accept snapshot changes.
        r.add_redaction(".result.coral", "[coralversion]");
        r.add_redaction(".result.gitPath", "[gitpath]");
        r.add_redaction(".error.message", "[message]");
        r
    }};
}

#[tokio::test]
async fn open_reports_a_clean_repository() {
    let fixture = TestRepo::new().write("a.txt", "hello\n").commit("first");
    let out = coral_cli::run(argv(&[
        "--json",
        "--repo",
        fixture.path().to_str().unwrap(),
        "open",
    ]))
    .await;

    stable!().bind(|| insta::assert_json_snapshot!("open_clean", out.json));
}

#[tokio::test]
async fn open_reports_an_unborn_branch() {
    let fixture = TestRepo::new();
    let out = coral_cli::run(argv(&[
        "--json",
        "--repo",
        fixture.path().to_str().unwrap(),
        "open",
    ]))
    .await;

    stable!().bind(|| insta::assert_json_snapshot!("open_unborn", out.json));
}

#[tokio::test]
async fn open_rejects_a_directory_that_is_not_a_repository() {
    let dir = tempfile::tempdir().unwrap();
    let out = coral_cli::run(argv(&[
        "--json",
        "--repo",
        dir.path().to_str().unwrap(),
        "open",
    ]))
    .await;

    stable!().bind(|| insta::assert_json_snapshot!("open_not_a_repository", out.json));
}

#[tokio::test]
async fn version_reports_the_resolved_git() {
    let out = coral_cli::run(argv(&["--json", "version"])).await;

    // The CLI and the window have to report the same number, and both take it from the crate.
    assert_eq!(out.json["result"]["coral"], env!("CARGO_PKG_VERSION"));
    stable!().bind(|| insta::assert_json_snapshot!("version", out.json));
}

#[tokio::test]
async fn an_unknown_subcommand_is_a_usage_error() {
    let out = coral_cli::run(argv(&["--json", "bogus"])).await;

    assert_eq!(out.json["error"]["code"], "usage");
    assert_eq!(out.json["ok"], false);
    assert_eq!(out.json["schema"], 1);
}

/// The exit code is as much a part of the contract as the JSON, and the kernel scenarios in
/// tests/kernel branch on it.
#[tokio::test]
async fn exit_codes_match_the_documented_contract() {
    use std::process::ExitCode;

    let fixture = TestRepo::new().write("a.txt", "x\n").commit("first");
    let ok = coral_cli::run(argv(&["--repo", fixture.path().to_str().unwrap(), "open"])).await;
    assert_eq!(format!("{:?}", ok.code), format!("{:?}", ExitCode::from(0)));

    let dir = tempfile::tempdir().unwrap();
    let bad = coral_cli::run(argv(&["--repo", dir.path().to_str().unwrap(), "open"])).await;
    assert_eq!(
        format!("{:?}", bad.code),
        format!("{:?}", ExitCode::from(2))
    );
}

#[tokio::test]
async fn refs_lists_branches_and_tags_and_filters_by_kind() {
    let fixture = TestRepo::new().write("a.txt", "1\n").commit("first");
    fixture.git(["tag", "v1"]);
    fixture.git(["branch", "feature/x"]);
    let repo = fixture.path().to_str().unwrap();

    let all = coral_cli::run(argv(&["--json", "--repo", repo, "refs"])).await;
    assert_eq!(all.json["result"]["refs"].as_array().unwrap().len(), 3);

    let tags = coral_cli::run(argv(&["--json", "--repo", repo, "refs", "--kind", "tag"])).await;
    let tags = tags.json["result"]["refs"].as_array().unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0]["short"], "v1");

    let local = coral_cli::run(argv(&["--json", "--repo", repo, "refs", "--kind", "local"])).await;
    assert_eq!(local.json["result"]["refs"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn graph_reports_rows_lanes_and_merge_flags() {
    let fixture = TestRepo::new().write("a.txt", "1\n").commit("base");
    fixture.git(["checkout", "--quiet", "-b", "side"]);
    let fixture = fixture.write("s.txt", "s\n").commit("side");
    fixture.git(["checkout", "--quiet", "main"]);
    let fixture = fixture.write("m.txt", "m\n").commit("main");
    fixture.git(["merge", "--quiet", "--no-ff", "-m", "merge", "side"]);

    let out = coral_cli::run(argv(&[
        "--json",
        "--repo",
        fixture.path().to_str().unwrap(),
        "graph",
    ]))
    .await;
    let g = &out.json["result"];

    assert_eq!(g["total"], 4);
    assert_eq!(g["provisional"], false);
    assert_eq!(g["rows"][0]["merge"], true, "the tip is the merge");
    assert_eq!(g["rows"][3]["root"], true, "the last row is the root");
    assert!(
        g["lanes"].as_u64().unwrap() >= 2,
        "a merge needs at least two lanes"
    );
}

/// A parent outside the loaded rows must serialize as null, never as a sentinel integer that
/// a consumer would read as row 4294967295.
#[tokio::test]
async fn a_boundary_parent_serializes_as_null() {
    let fixture = TestRepo::new().write("a.txt", "1\n").commit("one");
    let fixture = fixture.write("b.txt", "2\n").commit("two");

    let out = coral_cli::run(argv(&[
        "--json",
        "--repo",
        fixture.path().to_str().unwrap(),
        "graph",
        "--limit",
        "1",
    ]))
    .await;
    let row = &out.json["result"]["rows"][0];

    assert_eq!(row["boundary"], true);
    assert!(
        row["parents"][0].is_null(),
        "an unloaded parent must be null"
    );
}

#[tokio::test]
async fn first_paint_rows_are_marked_provisional() {
    let fixture = TestRepo::new().write("a.txt", "1\n").commit("one");
    let out = coral_cli::run(argv(&[
        "--json",
        "--repo",
        fixture.path().to_str().unwrap(),
        "graph",
        "--first-paint",
    ]))
    .await;

    assert_eq!(out.json["result"]["provisional"], true);
}

#[tokio::test]
async fn status_reports_staged_and_untracked_entries() {
    let fixture = TestRepo::new().write("tracked.txt", "1\n").commit("base");
    let fixture = fixture
        .write("tracked.txt", "changed\n")
        .write("new.txt", "n\n");
    fixture.git(["add", "new.txt"]);

    let out = coral_cli::run(argv(&[
        "--json",
        "--repo",
        fixture.path().to_str().unwrap(),
        "status",
    ]))
    .await;
    let s = &out.json["result"];

    assert_eq!(s["branch"], "main");
    let paths: Vec<&str> = s["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&"tracked.txt"));
    assert!(paths.contains(&"new.txt"));
}

#[tokio::test]
async fn diff_reports_staged_and_unstaged_separately() {
    let fixture = TestRepo::new().write("a.txt", "1\n").commit("base");
    let fixture = fixture.write("a.txt", "staged\n");
    fixture.git(["add", "a.txt"]);
    let fixture = fixture.write("b.txt", "unstaged\n");
    let repo = fixture.path().to_str().unwrap();

    let staged = coral_cli::run(argv(&["--json", "--repo", repo, "diff", "--staged"])).await;
    let files = staged.json["result"]["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["path"], "a.txt");
    assert_eq!(files[0]["change"], "modified");
    assert!(!files[0]["hunks"].as_array().unwrap().is_empty());

    // b.txt is untracked, so it does not appear in a worktree diff at all.
    let unstaged = coral_cli::run(argv(&["--json", "--repo", repo, "diff"])).await;
    assert!(
        unstaged.json["result"]["files"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn log_reports_commits_newest_first() {
    let fixture = TestRepo::new().write("a.txt", "1\n").commit("older");
    let fixture = fixture.write("a.txt", "2\n").commit("newer");

    let out = coral_cli::run(argv(&[
        "--json",
        "--repo",
        fixture.path().to_str().unwrap(),
        "log",
    ]))
    .await;
    let commits = out.json["result"]["commits"].as_array().unwrap();

    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0]["summary"], "newer");
    assert_eq!(commits[1]["summary"], "older");
    assert_eq!(commits[0]["author"]["name"], "Coral Fixture");
}

#[tokio::test]
async fn blame_maps_lines_to_the_commits_that_wrote_them() {
    let fixture = TestRepo::new().write("f.txt", "a\nb\n").commit("first");
    let fixture = fixture.write("f.txt", "a\nCHANGED\n").commit("second");

    let out = coral_cli::run(argv(&[
        "--json",
        "--repo",
        fixture.path().to_str().unwrap(),
        "blame",
        "f.txt",
    ]))
    .await;
    let r = &out.json["result"];

    assert_eq!(r["chunks"].as_array().unwrap().len(), 2);
    assert_eq!(r["commits"].as_object().unwrap().len(), 2);
}

#[tokio::test]
async fn worktrees_lists_the_one_every_repository_has() {
    let fixture = TestRepo::new().write("a.txt", "a\n").commit("first");
    let out = coral_cli::run(argv(&[
        "--json",
        "--repo",
        fixture.path().to_str().unwrap(),
        "worktrees",
    ]))
    .await;

    let list = &out.json["result"]["worktrees"];
    assert_eq!(list.as_array().map(Vec::len), Some(1), "{}", out.json);
    assert_eq!(list[0]["branch"], "main");
    assert_eq!(out.json["ok"], true);
}

#[tokio::test]
async fn patch_writes_the_commit_it_was_asked_for() {
    let fixture = TestRepo::new()
        .write("a.txt", "a\n")
        .commit("first")
        .write("b.txt", "b\n")
        .commit("second");
    let out_dir = tempfile::tempdir().unwrap();
    let out = coral_cli::run(argv(&[
        "--json",
        "--repo",
        fixture.path().to_str().unwrap(),
        "patch",
        "--out",
        out_dir.path().to_str().unwrap(),
    ]))
    .await;

    let files = out.json["result"]["files"].as_array().cloned().unwrap();
    assert_eq!(files.len(), 1, "{}", out.json);
    let written = files[0].as_str().unwrap();
    assert!(
        std::path::Path::new(written)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("patch")),
        "{written}"
    );
    assert!(std::fs::read_to_string(written).unwrap().contains("second"));
}

#[tokio::test]
async fn a_rewrite_of_a_commit_off_the_branch_is_refused_with_a_code() {
    // The envelope's `code` is what the UI switches on, so it matters as much as the message.
    let fixture = TestRepo::new().write("a.txt", "a\n").commit("first");
    fixture.git(["checkout", "--quiet", "-b", "side"]);
    let elsewhere = fixture.git(["commit-tree", "-m", "orphan", "HEAD^{tree}"]);

    let out = coral_cli::run(argv(&[
        "--json",
        "--repo",
        fixture.path().to_str().unwrap(),
        "rewrite",
        &elsewhere,
        "--kind",
        "drop",
    ]))
    .await;

    assert_eq!(out.json["ok"], false, "{}", out.json);
    assert_eq!(out.json["error"]["code"], "refused");
}
