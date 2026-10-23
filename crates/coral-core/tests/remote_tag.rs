//! Asking a remote whether it holds a tag.

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

#[tokio::test]
async fn a_remote_has_a_tag_only_once_it_was_pushed_there() {
    let host = TestRepo::new().write("a.txt", "1\n").commit("start");
    let local = TestRepo::new();
    local.git(["remote", "add", "origin", host.path().to_str().unwrap()]);
    local.git(["fetch", "--quiet", "origin"]);
    local.git(["checkout", "--quiet", "-B", "main", "origin/main"]);
    local.git(["tag", "-a", "-m", "first", "v1"]);

    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, local.path()).await.unwrap();
    assert_eq!(loc.remote_tag(&runner, "origin", "v1").await.unwrap(), None);

    local.git(["push", "--quiet", "origin", "v1"]);
    let held = loc.remote_tag(&runner, "origin", "v1").await.unwrap();
    // The tag object, not the commit it points at: that is what the remote's ref holds.
    assert_eq!(held.as_deref(), Some(local.git(["rev-parse", "v1"]).trim()));
}
