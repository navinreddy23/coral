//! Adding, changing and removing a commit's note.

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

#[tokio::test]
async fn adds_changes_and_removes_a_note_without_touching_the_commit() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("start");
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    let head = repo.git(["rev-parse", "HEAD"]);

    assert_eq!(loc.note(&runner, "HEAD").await.unwrap(), None);

    loc.set_note(&runner, "HEAD", "-- reviewed by Ada\nand tested")
        .await
        .unwrap();
    assert_eq!(
        loc.note(&runner, "HEAD").await.unwrap().as_deref(),
        Some("-- reviewed by Ada\nand tested")
    );
    assert_eq!(repo.git(["rev-parse", "HEAD"]), head);

    loc.set_note(&runner, "HEAD", "shipped in 1.2")
        .await
        .unwrap();
    assert_eq!(
        loc.note(&runner, "HEAD").await.unwrap().as_deref(),
        Some("shipped in 1.2")
    );

    loc.set_note(&runner, "HEAD", "  ").await.unwrap();
    assert_eq!(loc.note(&runner, "HEAD").await.unwrap(), None);
    // Removing a note that is not there is not an error.
    loc.set_note(&runner, "HEAD", "").await.unwrap();
}
