use coral_core::blame::BlameParser;
use coral_core::history::LogQuery;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

async fn open(repo: &TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

#[tokio::test]
async fn log_reads_metadata_including_multiline_bodies() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("first");
    let repo = repo.write("a.txt", "2\n");
    repo.git(["add", "--all"]);
    repo.git([
        "commit",
        "--quiet",
        "-m",
        "subject line",
        "-m",
        "body line one\nbody line two",
    ]);

    let (runner, loc) = open(&repo).await;
    let commits = loc.log(&runner, &LogQuery::default()).await.unwrap();

    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].summary, "subject line");
    assert_eq!(
        commits[0].body.to_string(),
        "body line one\nbody line two",
        "a body with newlines must not split the record"
    );
    assert_eq!(commits[0].author.name, "Coral Fixture");
    assert_eq!(commits[0].parents, vec![commits[1].oid.clone()]);
    assert!(commits[1].parents.is_empty(), "the root has no parents");
    assert!(!commits[0].is_merge());
}

#[tokio::test]
async fn log_records_both_parents_of_a_merge() {
    let repo = TestRepo::new().write("a.txt", "base\n").commit("base");
    repo.git(["checkout", "--quiet", "-b", "side"]);
    let repo = repo.write("s.txt", "s\n").commit("side");
    repo.git(["checkout", "--quiet", "main"]);
    let repo = repo.write("m.txt", "m\n").commit("main");
    repo.git(["merge", "--quiet", "--no-ff", "-m", "merge", "side"]);

    let (runner, loc) = open(&repo).await;
    let commits = loc.log(&runner, &LogQuery::default()).await.unwrap();
    assert!(commits[0].is_merge());
    assert_eq!(commits[0].parents.len(), 2);
}

#[tokio::test]
async fn log_narrows_by_limit_author_path_and_message() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("alpha");
    let repo = repo.write("b.txt", "1\n").commit("beta");
    let repo = repo.write("a.txt", "2\n").commit("gamma touches a");

    let (runner, loc) = open(&repo).await;

    let limited = loc
        .log(
            &runner,
            &LogQuery {
                limit: Some(2),
                ..LogQuery::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(limited.len(), 2);

    let by_path = loc
        .log(
            &runner,
            &LogQuery {
                path: Some("a.txt".into()),
                ..LogQuery::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(by_path.len(), 2, "only the commits touching a.txt");

    let by_grep = loc
        .log(
            &runner,
            &LogQuery {
                grep: Some("beta".into()),
                ..LogQuery::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(by_grep.len(), 1);

    let by_author = loc
        .log(
            &runner,
            &LogQuery {
                author: Some("Coral".into()),
                ..LogQuery::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(by_author.len(), 3);

    let missing = loc
        .log(
            &runner,
            &LogQuery {
                author: Some("nobody".into()),
                ..LogQuery::default()
            },
        )
        .await
        .unwrap();
    assert!(missing.is_empty());
}

#[tokio::test]
async fn blame_attributes_every_line_to_the_commit_that_wrote_it() {
    let repo = TestRepo::new()
        .write("f.txt", "one\ntwo\nthree\n")
        .commit("first");
    let first = repo.git(["rev-parse", "HEAD"]);
    let repo = repo
        .write("f.txt", "one\nCHANGED\nthree\n")
        .commit("second");
    let second = repo.git(["rev-parse", "HEAD"]);

    let (runner, loc) = open(&repo).await;
    let blame = loc.blame(&runner, "HEAD", "f.txt").await.unwrap();

    assert_eq!(blame.commit_for_line(1).unwrap().oid, first);
    assert_eq!(
        blame.commit_for_line(2).unwrap().oid,
        second,
        "the edited line"
    );
    assert_eq!(blame.commit_for_line(3).unwrap().oid, first);
    assert!(blame.commit_for_line(99).is_none());

    let c = blame.commit_for_line(2).unwrap();
    assert_eq!(c.author.name, "Coral Fixture");
    assert_eq!(c.summary, "second");
    assert_eq!(c.filename, "f.txt");
}

/// Git describes a commit once and then refers back to it by oid alone. A later chunk naming
/// the same commit must not blank out the details already gathered.
#[test]
fn a_repeated_commit_keeps_the_details_from_its_first_mention() {
    let mut p = BlameParser::default();
    for line in [
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1 1 1".as_bytes(),
        b"author Ada",
        b"author-mail <ada@example.com>",
        b"author-time 1500000000",
        b"summary first work",
        b"filename f.txt",
        // Same commit again, header and filename only.
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 5 9 2".as_bytes(),
        b"filename f.txt",
    ] {
        p.push(line).unwrap();
    }
    let blame = p.finish();

    assert_eq!(blame.chunks.len(), 2);
    assert_eq!(blame.commits.len(), 1, "one commit, described once");
    let c = blame.commit_for_line(9).unwrap();
    assert_eq!(
        c.author.name, "Ada",
        "details survived the bare second mention"
    );
    assert_eq!(
        c.author.email, "ada@example.com",
        "the angle brackets are stripped"
    );
}

#[test]
fn blame_chunks_come_back_in_file_order() {
    let mut p = BlameParser::default();
    for (oid, orig, fin, n) in [("b".repeat(40), 1, 10, 2), ("c".repeat(40), 1, 1, 3)] {
        p.push(format!("{oid} {orig} {fin} {n}").as_bytes())
            .unwrap();
        p.push(b"author X").unwrap();
        p.push(b"author-mail <x@y>").unwrap();
        p.push(b"author-time 1").unwrap();
        p.push(b"summary s").unwrap();
        p.push(b"filename f").unwrap();
    }
    let blame = p.finish();
    assert_eq!(
        blame.chunks[0].final_line, 1,
        "incremental output arrives out of order"
    );
    assert_eq!(blame.chunks[1].final_line, 10);
}

#[test]
fn metadata_before_any_header_is_rejected() {
    let mut p = BlameParser::default();
    assert!(p.push(b"author Nobody").is_err());
}

#[tokio::test]
async fn an_empty_history_is_not_an_error() {
    let repo = TestRepo::new();
    let (runner, loc) = open(&repo).await;
    // An unborn HEAD makes git log fail; the parser itself must handle emptiness.
    assert!(coral_core::history::parse(b"").unwrap().is_empty());
    assert!(loc.log(&runner, &LogQuery::default()).await.is_err());
}
