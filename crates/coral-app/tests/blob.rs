use coral_app_lib::blob::save_blob;
use coral_core::blob::BlobSide;
use coral_core::testutil::TestRepo;

#[tokio::test]
async fn saves_a_file_as_a_commit_had_it_byte_for_byte() {
    let repo = TestRepo::new()
        .write("a.bin", "first\0\u{ff}\n")
        .commit("first");
    let first = repo.git(["rev-parse", "HEAD"]).trim().to_owned();
    let repo = repo.write("a.bin", "second\n").commit("second");
    let out = tempfile::tempdir().unwrap();
    let to = out.path().join("saved.bin");

    save_blob(
        repo.path().to_str().unwrap().to_owned(),
        BlobSide::Commit { rev: first },
        "a.bin".to_owned(),
        to.to_str().unwrap().to_owned(),
    )
    .await
    .unwrap();
    assert_eq!(std::fs::read(&to).unwrap(), "first\0\u{ff}\n".as_bytes());

    // A side the file is not on is said to be one, rather than written as an empty file.
    let missing = save_blob(
        repo.path().to_str().unwrap().to_owned(),
        BlobSide::Commit {
            rev: "HEAD".to_owned(),
        },
        "nothing.txt".to_owned(),
        out.path().join("nothing.txt").to_str().unwrap().to_owned(),
    )
    .await;
    assert!(missing.is_err());
    assert!(!out.path().join("nothing.txt").exists());
}
