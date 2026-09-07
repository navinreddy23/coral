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
        ssh_key: None,
    };
    // The caller is told where it will land before it lands, so it can say so.
    assert_eq!(what.destination(), dir.path().join(name_from_url(&url)));

    let made = clone(&runner, &what, |_| {}).await.unwrap();
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
            ssh_key: None,
        },
        |_| {},
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
            ssh_key: None,
        },
        |_| {},
    )
    .await;

    assert!(matches!(
        refused,
        Err(coral_core::CoralError::AlreadyARepository(_))
    ));
    // And the repository that was there is untouched.
    assert!(existing.path().join("b.txt").exists());
}

#[tokio::test]
async fn a_clone_given_a_key_keeps_using_it_afterwards() {
    // The key has to outlive the clone. Passing it only through the environment would
    // authenticate the fetch of the clone itself and then leave the repository with nothing,
    // so the next pull would fall back to whichever key the agent offers first.
    let source = TestRepo::new().write("a.txt", "1\n").commit("base");
    let dir = tempfile::tempdir().unwrap();
    let runner = runner().await;

    let made = clone(
        &runner,
        &Cloned {
            url: format!("{}/.git", source.path().display()),
            parent: dir.path().to_path_buf(),
            name: Some("with-key".to_owned()),
            ssh_key: Some("/home/someone/.ssh/id_work".to_owned()),
        },
        |_| {},
    )
    .await
    .unwrap();

    let loc = RepoLocation::discover(&runner, &made).await.unwrap();
    let scopes = loc
        .ssh_scopes(&runner, coral_core::config::AppConfig::default())
        .await
        .unwrap();
    assert_eq!(
        scopes.local.private_key.as_deref(),
        Some("/home/someone/.ssh/id_work")
    );
    // And by the one builder, so `IdentitiesOnly=yes` cannot be lost to a second spelling.
    assert_eq!(
        scopes.effective.command,
        coral_core::ssh::command_for("/home/someone/.ssh/id_work")
    );
}

#[tokio::test]
async fn a_clone_given_no_key_writes_no_command() {
    // An empty `core.sshCommand` still shadows whatever the user configured by hand, so the
    // absence has to be a real absence.
    let source = TestRepo::new().write("a.txt", "1\n").commit("base");
    let dir = tempfile::tempdir().unwrap();
    let runner = runner().await;

    let made = clone(
        &runner,
        &Cloned {
            url: format!("{}/.git", source.path().display()),
            parent: dir.path().to_path_buf(),
            name: Some("no-key".to_owned()),
            ssh_key: None,
        },
        |_| {},
    )
    .await
    .unwrap();

    let written = std::fs::read_to_string(made.join(".git/config")).unwrap();
    assert!(!written.contains("sshCommand"), "{written}");
}

#[tokio::test]
async fn a_clone_reports_progress_rather_than_going_quiet() {
    // The point of streaming. A clone of anything large used to be a buffered wait with no
    // sign of life, and the counters git already writes were read by nobody.
    let source = TestRepo::new().write("a.txt", "1\n").commit("base");
    for n in 0..40 {
        std::fs::write(source.path().join(format!("f{n}.txt")), format!("{n}\n")).unwrap();
    }
    source.git(["add", "-A"]);
    source.git(["commit", "-qm", "many files"]);

    let dir = tempfile::tempdir().unwrap();
    let runner = runner().await;
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let collect = std::sync::Arc::clone(&seen);

    clone(
        &runner,
        &Cloned {
            // `file://` rather than a path: a plain local clone hardlinks the object store and
            // counts nothing, so it would prove only that the callback compiles.
            url: format!("file://{}/.git", source.path().display()),
            parent: dir.path().to_path_buf(),
            name: Some("watched".to_owned()),
            ssh_key: None,
        },
        move |p| collect.lock().unwrap().push(p),
    )
    .await
    .unwrap();

    let progress = seen.lock().unwrap();
    assert!(!progress.is_empty(), "no progress was reported at all");
    // Every record has to be usable as a fraction, or a bar drawn from it is a lie.
    assert!(
        progress.iter().all(|p| p.total > 0 && p.current <= p.total),
        "{progress:?}"
    );
    assert!(
        progress.iter().any(|p| p.percent == 100),
        "the last record of a phase should reach the end: {progress:?}"
    );
}

#[tokio::test]
async fn a_cancelled_clone_leaves_nothing_behind() {
    // Cancelling is dropping the future, which kills git where it stands. Nothing after the
    // await runs, so the half-made repository is removed by a guard rather than by cleanup
    // code that never gets to execute.
    let source = TestRepo::new().write("a.txt", "1\n").commit("base");
    let dir = tempfile::tempdir().unwrap();
    let runner = runner().await;
    let into = dir.path().join("abandoned");

    let what = Cloned {
        url: format!("{}/.git", source.path().display()),
        parent: dir.path().to_path_buf(),
        name: Some("abandoned".to_owned()),
        ssh_key: None,
    };
    let work = clone(&runner, &what, |_| {});
    // Dropped without ever being polled to completion, which is what an abort does.
    drop(work);

    assert!(
        !into.exists(),
        "a cancelled clone left {} behind",
        into.display()
    );
}

#[tokio::test]
async fn a_clone_into_a_directory_that_is_already_there_does_not_delete_it() {
    // The guard must never take work that was not ours. git refuses to clone into a non-empty
    // directory, and removing it because the clone failed would destroy whatever was in it.
    let source = TestRepo::new().write("a.txt", "1\n").commit("base");
    let dir = tempfile::tempdir().unwrap();
    let occupied = dir.path().join("occupied");
    std::fs::create_dir_all(&occupied).unwrap();
    std::fs::write(occupied.join("theirs.txt"), "not ours\n").unwrap();

    let runner = runner().await;
    let refused = clone(
        &runner,
        &Cloned {
            url: format!("{}/.git", source.path().display()),
            parent: dir.path().to_path_buf(),
            name: Some("occupied".to_owned()),
            ssh_key: None,
        },
        |_| {},
    )
    .await;

    assert!(
        refused.is_err(),
        "git should refuse a non-empty destination"
    );
    assert!(
        occupied.join("theirs.txt").exists(),
        "the directory that was already there must survive"
    );
}
