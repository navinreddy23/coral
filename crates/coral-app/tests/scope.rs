//! What a repository is showing in its graph, and what survives a restart.

use coral_app_lib::scope::{RepoScope, Scopes, pruned};
use coral_core::graph::Tips;
use coral_core::testutil::TestRepo;

#[test]
fn solo_wins_over_hiding() {
    // The two cannot both be in force: a walk says "only this one" or "all but these", never
    // both, and a window honouring both would have to explain which was winning.
    let scope = RepoScope {
        solo: Some("refs/heads/main".into()),
        hidden: vec!["refs/heads/spike".into()],
    };
    assert_eq!(scope.tips(), Tips::Only(vec!["refs/heads/main".into()]));
}

#[test]
fn hiding_nothing_is_the_whole_graph() {
    // `Tips::All`, not `Except(vec![])`. The same walk by a longer route, and it would defeat
    // the shortcut that keeps an unnarrowed repository off the disk and out of a rewalk.
    assert_eq!(RepoScope::default().tips(), Tips::All);
    assert!(RepoScope::default().is_everything());
}

#[test]
fn hiding_something_is_every_other_ref() {
    let scope = RepoScope {
        solo: None,
        hidden: vec!["refs/tags/v1".into()],
    };
    assert_eq!(scope.tips(), Tips::Except(vec!["refs/tags/v1".into()]));
    assert!(!scope.is_everything());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_branch_that_has_gone_is_dropped_rather_than_walked_from() {
    // A branch soloed yesterday and deleted this morning walks from nothing, and an empty
    // graph reads as the application having broken rather than as a branch having gone. The
    // banner naming a branch nobody can find is the same fault one layer up.
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    repo.git(["branch", "spike"]);

    let runner = coral_core::process::GitRunner::discover().await.unwrap();
    let loc = coral_core::repo::RepoLocation::discover(&runner, repo.path())
        .await
        .unwrap();
    let refs = loc.refs(&runner).await.unwrap();

    let stale = RepoScope {
        solo: Some("refs/heads/gone".into()),
        hidden: vec!["refs/heads/spike".into(), "refs/heads/also-gone".into()],
    };
    let live = pruned(&stale, &refs);

    assert_eq!(live.solo, None, "a solo naming a ref that is not there");
    assert_eq!(
        live.hidden,
        vec!["refs/heads/spike".to_owned()],
        "the one that is still there stays hidden"
    );
}

#[test]
fn a_repository_nobody_narrowed_is_not_written_down() {
    // The file is meant to be readable by hand, and an entry for every repository ever opened
    // — each of them empty — is not.
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("scope.json");
    let scopes = Scopes::load(file.clone());

    scopes.set("/one", RepoScope::default());
    scopes.set(
        "/two",
        RepoScope {
            solo: Some("refs/heads/x".into()),
            hidden: vec![],
        },
    );

    let written = std::fs::read_to_string(&file).unwrap();
    assert!(written.contains("/two"), "{written}");
    assert!(!written.contains("/one"), "{written}");
}

#[test]
fn showing_everything_again_takes_the_entry_back_out() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("scope.json");
    let scopes = Scopes::load(file.clone());

    scopes.set(
        "/repo",
        RepoScope {
            solo: Some("refs/heads/x".into()),
            hidden: vec![],
        },
    );
    scopes.set("/repo", RepoScope::default());

    assert_eq!(scopes.read("/repo"), RepoScope::default());
    assert!(!std::fs::read_to_string(&file).unwrap().contains("/repo"));
}

#[test]
fn what_was_narrowed_survives_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("scope.json");
    let want = RepoScope {
        solo: None,
        hidden: vec!["refs/heads/spike".into()],
    };

    Scopes::load(file.clone()).set("/repo", want.clone());
    assert_eq!(Scopes::load(file).read("/repo"), want);
}

#[test]
fn a_scope_file_that_cannot_be_read_leaves_every_repository_showing_everything() {
    // A hand-edited or truncated file must not stop the window opening, and the safe answer is
    // the whole graph rather than a guess at what was narrowed.
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("scope.json");
    std::fs::write(&file, b"{ not json").unwrap();

    assert_eq!(Scopes::load(file).read("/repo"), RepoScope::default());
}
