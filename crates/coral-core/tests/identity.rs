//! Who a repository records as the author of its commits.
//!
//! The property that matters is that a profile's identity lands in the repository's own
//! config and nowhere else: a user who looks at `.git/config`, or runs `git config user.email`
//! in their terminal, must see exactly what Coral wrote.

use coral_core::config::AppConfig;
use coral_core::identity::Identity;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

/// A repository and an app-level config file of its own, so nothing reads this machine's.
fn hermetic() -> (TestRepo, tempfile::TempDir) {
    let home = tempfile::tempdir().unwrap();
    std::fs::write(home.path().join("gitconfig"), "").unwrap();
    let repo = TestRepo::new().write("a.txt", "a").commit("first");
    (repo, home)
}

#[tokio::test(flavor = "multi_thread")]
async fn what_is_written_is_what_git_reports() {
    let (repo, _home) = hermetic();
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();

    loc.set_identity_local(
        &runner,
        &Identity {
            name: Some("Work Me".to_owned()),
            email: Some("me@work.example".to_owned()),
        },
    )
    .await
    .unwrap();

    // git itself, not our own reader, so a key written under the wrong name would show.
    assert_eq!(repo.git(["config", "--local", "user.name"]), "Work Me");
    assert_eq!(
        repo.git(["config", "--local", "user.email"]),
        "me@work.example"
    );

    let scopes = loc
        .identity_scopes(&runner, AppConfig::default())
        .await
        .unwrap();
    assert_eq!(scopes.local.name.as_deref(), Some("Work Me"));
    assert_eq!(scopes.effective.email.as_deref(), Some("me@work.example"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_field_left_out_clears_the_override_rather_than_emptying_it() {
    // An empty `user.email` is not the same as no `user.email`: git refuses to commit with the
    // first and falls back to the app level with the second, which is what "inherit" means
    // everywhere else in the engine.
    let (repo, home) = hermetic();
    let app_path = home.path().join("gitconfig");
    let app = AppConfig(Some(&app_path));
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();

    loc.set_identity_global(
        &runner,
        app,
        &Identity {
            name: Some("Default Me".to_owned()),
            email: Some("me@home.example".to_owned()),
        },
    )
    .await
    .unwrap();
    loc.set_identity_local(
        &runner,
        &Identity {
            name: Some("Work Me".to_owned()),
            email: None,
        },
    )
    .await
    .unwrap();

    let scopes = loc.identity_scopes(&runner, app).await.unwrap();
    assert_eq!(scopes.local.name.as_deref(), Some("Work Me"));
    assert_eq!(scopes.local.email, None, "the override is gone, not blank");
    assert_eq!(
        scopes.effective.email.as_deref(),
        Some("me@home.example"),
        "so the app level answers"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_app_level_is_left_alone_by_a_repository_write() {
    // The whole point of writing per repository is that the user's own git configuration is
    // not quietly rewritten under them.
    let (repo, home) = hermetic();
    let app_path = home.path().join("gitconfig");
    let app = AppConfig(Some(&app_path));
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();

    loc.set_identity_global(
        &runner,
        app,
        &Identity {
            name: Some("Default Me".to_owned()),
            email: Some("me@home.example".to_owned()),
        },
    )
    .await
    .unwrap();
    loc.set_identity_local(
        &runner,
        &Identity {
            name: Some("Work Me".to_owned()),
            email: Some("me@work.example".to_owned()),
        },
    )
    .await
    .unwrap();

    let scopes = loc.identity_scopes(&runner, app).await.unwrap();
    assert_eq!(scopes.global.name.as_deref(), Some("Default Me"));
    assert_eq!(scopes.global.email.as_deref(), Some("me@home.example"));
    assert_eq!(scopes.effective.name.as_deref(), Some("Work Me"));
}

#[test]
fn an_identity_that_sets_nothing_says_so() {
    assert!(Identity::default().is_empty());
    assert!(
        !Identity {
            name: None,
            email: Some("me@work.example".to_owned()),
        }
        .is_empty()
    );
}
