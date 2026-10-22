use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

async fn open(repo: &TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

#[tokio::test]
async fn a_hard_reset_leaves_the_commits_it_dropped_marked_lost() {
    let repo = TestRepo::new()
        .write("a.txt", "a\n")
        .commit("one")
        .write("a.txt", "b\n")
        .commit("two")
        .write("a.txt", "c\n")
        .commit("three");
    repo.git(["reset", "--quiet", "--hard", "HEAD~2"]);
    let (runner, loc) = open(&repo).await;

    let entries = loc.reflog(&runner, 50).await.unwrap();
    let seen: Vec<(&str, bool)> = entries
        .iter()
        .map(|e| (e.summary.as_str(), e.lost))
        .collect();
    assert_eq!(
        seen,
        [
            ("one", false),
            ("three", true),
            ("two", true),
            ("one", false)
        ]
    );
    assert!(
        entries[0].action.starts_with("reset: moving to"),
        "{}",
        entries[0].action
    );
    assert!(entries[0].time > 0);
}

#[tokio::test]
async fn a_deleted_branch_s_commits_are_lost_and_a_kept_one_s_are_not() {
    let repo = TestRepo::new().write("a.txt", "a\n").commit("start");
    repo.git(["checkout", "--quiet", "-b", "kept"]);
    let repo = repo.write("k.txt", "k\n").commit("on kept");
    repo.git(["checkout", "--quiet", "-b", "gone", "main"]);
    let repo = repo.write("g.txt", "g\n").commit("on gone");
    repo.git(["checkout", "--quiet", "main"]);
    repo.git(["branch", "--quiet", "-D", "gone"]);
    let (runner, loc) = open(&repo).await;

    let entries = loc.reflog(&runner, 50).await.unwrap();
    let lost = |summary: &str| entries.iter().find(|e| e.summary == summary).unwrap().lost;
    assert!(lost("on gone"));
    assert!(!lost("on kept"));
    assert!(!lost("start"));
}

#[tokio::test]
async fn reads_no_further_back_than_asked() {
    let mut repo = TestRepo::new();
    for n in 0..5 {
        repo = repo
            .write("a.txt", &format!("{n}\n"))
            .commit(&format!("c{n}"));
    }
    let (runner, loc) = open(&repo).await;
    let entries = loc.reflog(&runner, 3).await.unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].summary, "c4");
}

#[tokio::test]
async fn a_repository_with_no_commits_has_nothing_to_recover() {
    let repo = TestRepo::new();
    let (runner, loc) = open(&repo).await;
    assert_eq!(loc.reflog(&runner, 50).await.unwrap().len(), 0);
}
