//! Making a repository, against real git.
//!
//! Nothing here reaches the network: a clone is made from a second repository on disk, which
//! exercises the same command and the same destination rule without depending on a host being
//! up.

use coral_core::create::{Cloned, NewRepo, clone, init, name_from_url};
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

async fn runner() -> GitRunner {
    GitRunner::discover().await.unwrap()
}

#[tokio::test]
async fn creates_a_repository_git_can_open() {
    let dir = tempfile::tempdir().unwrap();
    let runner = runner().await;
    let where_ = dir.path().join("fresh");

    let made = init(
        &runner,
        &NewRepo {
            path: where_.clone(),
            branch: Some("main".to_owned()),
            lfs: false,
        },
    )
    .await
    .unwrap();

    assert_eq!(made, where_);
    let loc = RepoLocation::discover(&runner, &made).await.unwrap();
    let info = loc.info(&runner).await.unwrap();
    assert!(matches!(info.head, coral_core::repo::Head::Unborn { .. }));
}

#[tokio::test]
async fn puts_the_first_branch_where_it_was_asked_to() {
    let dir = tempfile::tempdir().unwrap();
    let runner = runner().await;
    let path = dir.path().join("named");

    init(
        &runner,
        &NewRepo {
            path: path.clone(),
            branch: Some("trunk".to_owned()),
            lfs: false,
        },
    )
    .await
    .unwrap();

    let head = std::fs::read_to_string(path.join(".git/HEAD")).unwrap();
    assert!(head.contains("refs/heads/trunk"), "{head}");
}

#[tokio::test]
async fn refuses_to_create_one_on_top_of_another() {
    // git would answer "reinitialized existing repository", which is a success message for
    // something nobody asked for.
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let runner = runner().await;

    let refused = init(
        &runner,
        &NewRepo {
            path: repo.path().to_path_buf(),
            branch: None,
            lfs: false,
        },
    )
    .await;

    assert!(matches!(
        refused,
        Err(coral_core::CoralError::AlreadyARepository(_))
    ));
}

#[tokio::test]
async fn clones_into_the_directory_git_would_have_chosen() {
    let source = TestRepo::new().write("a.txt", "1\n").commit("base");
    let dir = tempfile::tempdir().unwrap();
    let runner = runner().await;

    let url = format!("{}/.git", source.path().display());
    let what = Cloned {
        url: url.clone(),
        parent: dir.path().to_path_buf(),
        name: None,
    };
    // The caller is told where it will land before it lands, so it can say so.
    assert_eq!(what.destination(), dir.path().join(name_from_url(&url)));

    let made = clone(&runner, &what).await.unwrap();
    assert_eq!(made, what.destination());
    assert!(made.join("a.txt").exists());
}

#[tokio::test]
async fn clones_under_the_name_it_was_given() {
    let source = TestRepo::new().write("a.txt", "1\n").commit("base");
    let dir = tempfile::tempdir().unwrap();
    let runner = runner().await;

    let made = clone(
        &runner,
        &Cloned {
            url: format!("{}/.git", source.path().display()),
            parent: dir.path().to_path_buf(),
            name: Some("called-this".to_owned()),
        },
    )
    .await
    .unwrap();

    assert_eq!(made, dir.path().join("called-this"));
    assert!(made.join("a.txt").exists());
}

#[tokio::test]
async fn refuses_to_clone_over_a_repository_that_is_already_there() {
    let source = TestRepo::new().write("a.txt", "1\n").commit("base");
    let existing = TestRepo::new().write("b.txt", "1\n").commit("base");
    let runner = runner().await;

    let refused = clone(
        &runner,
        &Cloned {
            url: format!("{}/.git", source.path().display()),
            parent: existing.path().parent().unwrap().to_path_buf(),
            name: Some(
                existing
                    .path()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            ),
        },
    )
    .await;

    assert!(matches!(
        refused,
        Err(coral_core::CoralError::AlreadyARepository(_))
    ));
    // And the repository that was there is untouched.
    assert!(existing.path().join("b.txt").exists());
}
