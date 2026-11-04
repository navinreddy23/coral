//! Starting the configured diff and merge tools. The tools here are shell lines in the
//! fixture's own config, so what is checked is what git actually ran and what it left behind.

use std::time::{Duration, Instant};

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;
use coral_core::tool::Compared;

async fn open(repo: &TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

/// Waits for a launched tool to have done its work, since nothing waits on it by design.
fn eventually(what: &str, done: impl Fn() -> bool) {
    let until = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < until, "{what} never happened");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[tokio::test]
async fn nothing_is_started_without_a_tool_configured() {
    let repo = TestRepo::new().write("a.txt", "a\n").commit("base");
    let (runner, loc) = open(&repo).await;
    assert_eq!(loc.external_tools(&runner).await.unwrap().diff, None);
    let e = loc
        .open_difftool(&runner, "a.txt", &Compared::Unstaged)
        .await
        .unwrap_err();
    assert_eq!(e.code(), "refused");
}

#[tokio::test]
async fn the_diff_tool_is_handed_both_sides_of_one_commit() {
    let repo = TestRepo::new().write("a.txt", "old\n").commit("base");
    let repo = repo.write("a.txt", "new\n").commit("change");
    let seen = repo.path().join("seen.txt");
    repo.git(["config", "diff.tool", "fake"]);
    repo.git([
        "config",
        "difftool.fake.cmd",
        // Forward slashes, which git's own shell on Windows reads as readily as `/` paths.
        &format!(
            "cat \"$LOCAL\" \"$REMOTE\" > '{}'",
            seen.display().to_string().replace('\\', "/")
        ),
    ]);
    let (runner, loc) = open(&repo).await;
    assert_eq!(
        loc.external_tools(&runner).await.unwrap().diff.as_deref(),
        Some("fake")
    );

    let head = repo.git(["rev-parse", "HEAD"]).trim().to_owned();
    loc.open_difftool(&runner, "a.txt", &Compared::Commit(head))
        .await
        .unwrap();
    eventually("the diff tool writing what it was given", || {
        std::fs::read_to_string(&seen).is_ok_and(|s| s == "old\nnew\n")
    });
}

#[tokio::test]
async fn the_merge_tool_resolves_and_git_stages_what_it_wrote() {
    let repo = TestRepo::new().write("f.txt", "base\n").commit("base");
    repo.git(["checkout", "--quiet", "-b", "side"]);
    let repo = repo.write("f.txt", "side\n").commit("side");
    repo.git(["checkout", "--quiet", "main"]);
    let repo = repo.write("f.txt", "main\n").commit("main");
    let _ = repo.command(["merge", "side"]).output().unwrap();
    repo.git(["config", "merge.tool", "fake"]);
    repo.git(["config", "mergetool.fake.cmd", "cp \"$REMOTE\" \"$MERGED\""]);
    repo.git(["config", "mergetool.fake.trustExitCode", "true"]);
    repo.git(["config", "mergetool.keepBackup", "false"]);

    let (runner, loc) = open(&repo).await;
    loc.open_mergetool(&runner, "f.txt").await.unwrap();
    eventually("git staging the merge tool's answer", || {
        repo.git(["ls-files", "-u"]).is_empty()
    });
    assert_eq!(repo.git(["show", ":f.txt"]), "side");
}
