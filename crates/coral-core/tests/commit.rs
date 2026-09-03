//! Reading one commit: what it says, and which files it changed.

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

#[test]
fn a_merge_lists_only_what_it_brought_in() {
    // `-m --first-parent` looks like it narrows to one parent and does not: diff-tree emits a
    // diff against each parent in turn, so this listed the mainline's own changes as well.
    let repo = TestRepo::new().write("base.txt", "base\n").commit("root");
    repo.git(["checkout", "-q", "-b", "side"]);
    let repo = repo.write("side.txt", "side\n").commit("on side");
    repo.git(["checkout", "-q", "-"]);
    let repo = repo.write("main.txt", "main\n").commit("on main");
    repo.git(["merge", "--no-ff", "-m", "merge side", "side"]);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let detail = loc.commit_detail(&runner, "HEAD").await.unwrap();
        let paths: Vec<_> = detail.files.iter().map(|f| f.path.to_string()).collect();
        assert_eq!(paths, ["side.txt"]);
    });
}
