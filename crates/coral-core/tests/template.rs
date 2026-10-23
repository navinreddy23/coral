//! Reading the commit message template a repository names.

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

async fn template(repo: &TestRepo) -> Option<String> {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    loc.commit_template(&runner).await.unwrap()
}

#[tokio::test]
async fn reads_a_template_named_relative_to_the_repository() {
    let repo = TestRepo::new()
        .write("a.txt", "1\n")
        .commit("start")
        .write(".gitmessage", "[TICKET-] \n\n# Why, not what.\n");
    repo.git(["config", "commit.template", ".gitmessage"]);
    assert_eq!(template(&repo).await.as_deref(), Some("[TICKET-]"));
}

#[tokio::test]
async fn no_template_and_a_missing_one_are_both_nothing() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("start");
    // Set to nothing here, whatever the developer's own config names.
    repo.git(["config", "commit.template", ""]);
    assert_eq!(template(&repo).await, None);

    repo.git(["config", "commit.template", "no-such-file"]);
    assert_eq!(template(&repo).await, None);
}
