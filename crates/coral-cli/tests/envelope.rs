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
