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

    stable!().bind(|| {
        insta::assert_json_snapshot!("open_clean", out.json);
    });
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

    stable!().bind(|| {
        insta::assert_json_snapshot!("open_unborn", out.json);
    });
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

    stable!().bind(|| {
        insta::assert_json_snapshot!("open_not_a_repository", out.json);
    });
}

#[tokio::test]
async fn version_reports_the_resolved_git() {
    let out = coral_cli::run(argv(&["--json", "version"])).await;

    // The CLI and the window have to report the same number, and both take it from the crate.
    assert_eq!(out.json["result"]["coral"], env!("CARGO_PKG_VERSION"));
    stable!().bind(|| {
        insta::assert_json_snapshot!("version", out.json);
    });
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
    let hunks = files[0]["hunks"].as_array().unwrap();
    assert!(!hunks.is_empty(), "{hunks:?}");

    // b.txt is untracked, so it does not appear in a worktree diff at all.
    let unstaged = coral_cli::run(argv(&["--json", "--repo", repo, "diff"])).await;
    let files = unstaged.json["result"]["files"].as_array().unwrap();
    assert!(files.is_empty(), "{files:?}");
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
async fn find_answers_like_the_find_box_including_what_a_change_contained() {
    let fixture = TestRepo::new().write("f.txt", "plain\n").commit("first");
    let fixture = fixture
        .write("f.txt", "plain\nneedle_here\n")
        .commit("adds a line");
    let fixture = fixture.write("g.txt", "x\n").commit("mentions needle_here");

    let by_message = coral_cli::run(argv(&[
        "--json",
        "--repo",
        fixture.path().to_str().unwrap(),
        "find",
        "needle_here",
    ]))
    .await;
    let found = by_message.json["result"]["commits"].as_array().unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0]["summary"], "mentions needle_here");

    let by_change = coral_cli::run(argv(&[
        "--json",
        "--repo",
        fixture.path().to_str().unwrap(),
        "find",
        "code:needle_here",
    ]))
    .await;
    let found = by_change.json["result"]["commits"].as_array().unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0]["summary"], "adds a line");
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

#[tokio::test]
async fn auto_fetch_shows_sets_and_hands_back_the_interval() {
    let fixture = TestRepo::new().write("a.txt", "1\n").commit("first");
    let repo = fixture.path().to_str().unwrap();
    let run = |extra: &'static [&'static str]| {
        let mut args = vec!["--json", "--repo", repo, "auto-fetch"];
        args.extend_from_slice(extra);
        coral_cli::run(argv(&args))
    };

    let shown = run(&[]).await;
    assert_eq!(
        shown.json["result"]["setting"],
        serde_json::Value::Null,
        "{}",
        shown.json
    );

    let every = run(&["--every", "3"]).await;
    assert_eq!(
        every.json["result"]["setting"]["kind"], "every",
        "{}",
        every.json
    );
    assert_eq!(every.json["result"]["setting"]["minutes"], 3);

    let off = run(&["--off"]).await;
    assert_eq!(off.json["result"]["setting"]["kind"], "off");

    let refused = run(&["--every", "0"]).await;
    assert_eq!(refused.json["error"]["code"], "refused", "{}", refused.json);

    let back = run(&["--default"]).await;
    assert_eq!(back.json["result"]["setting"], serde_json::Value::Null);
}

#[tokio::test]
async fn compare_and_diff_since_answer_like_the_window() {
    let fixture = TestRepo::new().write("a.txt", "1\n").commit("base");
    fixture.git(["checkout", "--quiet", "-b", "side"]);
    let fixture = fixture.write("side.txt", "s\n").commit("on the side");
    fixture.git(["checkout", "--quiet", "main"]);
    let fixture = fixture.write("main.txt", "m\n").commit("on main");
    let path = fixture.path().to_str().unwrap().to_owned();
    let repo = path.as_str();
    let paths = |out: &serde_json::Value| -> Vec<String> {
        let mut found: Vec<String> = out["result"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["path"].as_str().unwrap().to_owned())
            .collect();
        found.sort();
        found
    };

    let tips = coral_cli::run(argv(&["--json", "--repo", repo, "compare", "main", "side"])).await;
    assert_eq!(paths(&tips.json), ["main.txt", "side.txt"], "{}", tips.json);
    let since = coral_cli::run(argv(&[
        "--json",
        "--repo",
        repo,
        "compare",
        "main",
        "side",
        "--merge-base",
    ]))
    .await;
    assert_eq!(paths(&since.json), ["side.txt"], "{}", since.json);
    assert!(since.json["result"]["base"].is_string());

    let fixture = fixture.write("a.txt", "2\n");
    let worktree = coral_cli::run(argv(&[
        "--json", "--repo", repo, "diff", "--since", "HEAD~1",
    ]))
    .await;
    assert_eq!(
        paths(&worktree.json),
        ["a.txt", "main.txt"],
        "{}",
        worktree.json
    );
    drop(fixture);
}

#[tokio::test]
async fn take_writes_a_file_as_a_commit_or_a_stash_has_it_and_stages_nothing() {
    let fixture = TestRepo::new().write("a.txt", "first\n").commit("first");
    let fixture = fixture.write("a.txt", "second\n").commit("second");
    let path = fixture.path().to_str().unwrap().to_owned();
    let repo = path.as_str();

    let out = coral_cli::run(argv(&[
        "--json", "--repo", repo, "take", "HEAD~1", "--path", "a.txt",
    ]))
    .await;
    assert_eq!(out.json["ok"], true, "{}", out.json);
    assert_eq!(
        std::fs::read_to_string(fixture.path().join("a.txt")).unwrap(),
        "first\n"
    );
    assert_eq!(fixture.git(["diff", "--cached", "--name-only"]).trim(), "");

    fixture.git(["stash", "push", "--quiet"]);
    let out = coral_cli::run(argv(&[
        "--json", "--repo", repo, "take", "--stash", "0", "--path", "a.txt",
    ]))
    .await;
    assert_eq!(out.json["ok"], true, "{}", out.json);
    assert_eq!(
        std::fs::read_to_string(fixture.path().join("a.txt")).unwrap(),
        "first\n"
    );
}

#[tokio::test]
async fn a_stash_becomes_a_branch_from_the_command_line() {
    let fixture = TestRepo::new().write("a.txt", "base\n").commit("base");
    let fixture = fixture.write("a.txt", "half\n");
    fixture.git(["stash", "push", "--quiet"]);
    let path = fixture.path().to_str().unwrap().to_owned();

    let refused = coral_cli::run(argv(&["--json", "--repo", &path, "stash", "branch"])).await;
    assert_ne!(
        refused.json["ok"], true,
        "a branch needs a name: {}",
        refused.json
    );

    let out = coral_cli::run(argv(&[
        "--json", "--repo", &path, "stash", "branch", "--name", "half",
    ]))
    .await;
    assert_eq!(out.json["ok"], true, "{}", out.json);
    assert_eq!(fixture.git(["branch", "--show-current"]).trim(), "half");
}

#[tokio::test]
async fn merged_lists_what_can_go_and_unset_upstream_stops_tracking() {
    let fixture = TestRepo::new().write("a.txt", "1\n").commit("base");
    fixture.git(["branch", "done"]);
    fixture.git(["branch", "--track", "follower", "main"]);
    let path = fixture.path().to_str().unwrap().to_owned();

    let merged = coral_cli::run(argv(&["--json", "--repo", &path, "merged"])).await;
    let mut names: Vec<String> = merged.json["result"]["branches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b.as_str().unwrap().to_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["done", "follower"], "{}", merged.json);

    let out = coral_cli::run(argv(&[
        "--json",
        "--repo",
        &path,
        "unset-upstream",
        "follower",
    ]))
    .await;
    assert_eq!(out.json["ok"], true, "{}", out.json);
    assert!(
        !fixture
            .git(["config", "--list"])
            .contains("branch.follower.merge"),
        "follower follows nothing now"
    );
}
