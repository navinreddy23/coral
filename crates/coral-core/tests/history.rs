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
    assert!(missing.is_empty(), "{missing:?}");
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
    let blame = loc.blame(&runner, "HEAD", "f.txt", None).await.unwrap();

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
    let commits = coral_core::history::parse(b"").unwrap();
    assert!(commits.is_empty(), "{commits:?}");
    assert!(loc.log(&runner, &LogQuery::default()).await.is_err());
}

/// A search box takes one word and has to find it wherever it is: the message, the author, or
/// the object id. git ANDs `--author` with `--grep`, so one invocation cannot do it.
#[test]
fn searching_matches_the_message_the_author_or_the_id() {
    let repo = TestRepo::new()
        .write("a.txt", "1\n")
        .commit("teach the parser to count")
        .write("b.txt", "2\n")
        .commit("unrelated work");

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();

        let by_message = loc.search_commits(&runner, "parser", 50).await.unwrap();
        assert_eq!(by_message.len(), 1, "one commit mentions the parser");

        // The fixture's author, which appears in no message at all.
        let by_author = loc.search_commits(&runner, "Fixture", 50).await.unwrap();
        assert_eq!(by_author.len(), 2, "both commits, by their author");

        // Case-insensitive, and matched as text rather than as a pattern.
        assert_eq!(
            loc.search_commits(&runner, "PARSER", 50)
                .await
                .unwrap()
                .len(),
            1
        );
        let found = loc.search_commits(&runner, "par.er", 50).await.unwrap();
        assert!(found.is_empty(), "{found:?}");

        // An abbreviated object id names that commit and comes back first.
        let head = &by_message[0];
        let by_oid = loc.search_commits(&runner, &head[..8], 50).await.unwrap();
        assert_eq!(by_oid.first(), Some(head));

        // A hash nothing answers to is an empty result, not a failure.
        let found = loc
            .search_commits(&runner, "0123456789abcdef", 50)
            .await
            .unwrap();
        assert!(found.is_empty(), "{found:?}");
        let found = loc.search_commits(&runner, "   ", 50).await.unwrap();
        assert!(found.is_empty(), "{found:?}");
    });
}

/// A search that matched every commit would be no help and would cost a full walk.
#[test]
fn searching_stops_at_the_limit() {
    let mut repo = TestRepo::new()
        .write("a.txt", "0\n")
        .commit("common word 0");
    for i in 1..12 {
        repo = repo
            .write("a.txt", &format!("{i}\n"))
            .commit(&format!("common word {i}"));
    }

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        assert_eq!(
            loc.search_commits(&runner, "common", 5)
                .await
                .unwrap()
                .len(),
            5
        );
    });
}

/// The fourth thing a filter is asked: which commits touched this file.
///
/// A pathspec rather than `-S`. `-S` searches the content of every diff, which answers a
/// different question and takes minutes on a real repository.
#[test]
fn searching_matches_a_file_path() {
    let repo = TestRepo::new()
        .write("src/parser.rs", "1\n")
        .commit("first")
        .write("docs/guide.md", "2\n")
        .commit("second")
        .write("src/parser.rs", "3\n")
        .commit("third");

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();

        // Two commits touched that file, and no message or author mentions it.
        let by_path = loc.search_commits(&runner, "parser.rs", 50).await.unwrap();
        assert_eq!(by_path.len(), 2, "both commits that touched the parser");

        // Any part of the path, not the whole of it, and not only the leaf.
        assert_eq!(
            loc.search_commits(&runner, "docs/", 50)
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            loc.search_commits(&runner, "guide", 50)
                .await
                .unwrap()
                .len(),
            1
        );

        // The same case-insensitivity the other passes have.
        assert_eq!(
            loc.search_commits(&runner, "PARSER.RS", 50)
                .await
                .unwrap()
                .len(),
            2
        );

        // A path nothing has is not a match, and not an error either.
        let found = loc
            .search_commits(&runner, "nowhere.txt", 50)
            .await
            .unwrap();
        assert!(found.is_empty(), "{found:?}");
    });
}

#[tokio::test]
async fn blame_reaches_a_file_by_the_name_it_had_then() {
    // Stepping back through a file's history and asking who wrote a line answered "no such
    // path <current name> in <forty characters>": blame takes one path and one revision, and
    // before the rename the current name is not in that tree.
    let repo = TestRepo::new().write("before.txt", "one\n").commit("base");
    let base = repo.git(["rev-parse", "HEAD"]).trim().to_owned();
    repo.git(["mv", "before.txt", "after.txt"]);
    let repo = repo.commit("moved");

    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();

    let blame = loc
        .blame(&runner, &base, "after.txt", Some("before.txt"))
        .await
        .unwrap();
    assert_eq!(blame.chunks.len(), 1, "the file as it was named then");

    let text = loc
        .file_at(&runner, &base, "after.txt", Some("before.txt"))
        .await
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&text), "one\n");

    // And the current name still wins wherever it exists.
    let now = loc
        .file_at(&runner, "HEAD", "after.txt", Some("before.txt"))
        .await
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&now), "one\n");
}

/// A commit made in the fixture at `date`, rather than at the fixture's one fixed instant.
fn commit_dated(repo: &TestRepo, file: &str, contents: &str, message: &str, date: &str) {
    std::fs::write(repo.path().join(file), contents).unwrap();
    repo.git(["add", "--all"]);
    let status = repo
        .command(["commit", "--quiet", "-m", message])
        .env("GIT_AUTHOR_DATE", date)
        .env("GIT_COMMITTER_DATE", date)
        .status()
        .unwrap();
    assert!(status.success());
}

/// `code:` finds the commits that changed the code, whatever their messages say.
#[tokio::test]
async fn code_finds_the_commits_that_added_or_removed_it() {
    let repo = TestRepo::new()
        .write("row.rs", "fn draw() {}\n")
        .commit("start")
        .write("row.rs", "fn draw() {}\nfn render_row() {}\n")
        .commit("more drawing")
        .write(
            "row.rs",
            "fn draw() { render_row(); }\nfn render_row() {}\n",
        )
        .commit("call it")
        .write("notes.txt", "render_row is slow\n")
        .commit("unrelated notes about render_row");
    let (runner, loc) = open(&repo).await;
    let subject = |oid: &str| {
        repo.git(["log", "-1", "--format=%s", oid])
            .trim()
            .to_owned()
    };

    // -S: the commit that introduced the definition, the one that added a call, and the note,
    // each of which changed how often the string occurs. Not the start.
    let found = loc
        .search_commits(&runner, "code:render_row", 50)
        .await
        .unwrap();
    let subjects: Vec<String> = found.iter().map(|o| subject(o)).collect();
    assert_eq!(
        subjects,
        [
            "unrelated notes about render_row",
            "call it",
            "more drawing"
        ]
    );

    // Words beside it narrow it: only the commits that changed it and say "call".
    let found = loc
        .search_commits(&runner, "call code:render_row", 50)
        .await
        .unwrap();
    assert_eq!(
        found.iter().map(|o| subject(o)).collect::<Vec<_>>(),
        ["call it"]
    );

    // A pattern, as -G, matched against the added and removed lines; quoted, for its space.
    let found = loc
        .search_commits(&runner, r#"code:"/fn [a-z_]+_row/""#, 50)
        .await
        .unwrap();
    assert_eq!(
        found.iter().map(|o| subject(o)).collect::<Vec<_>>(),
        ["more drawing"]
    );

    // Case is ignored here as it is everywhere else in the box.
    let upper = loc
        .search_commits(&runner, "code:RENDER_ROW", 50)
        .await
        .unwrap();
    assert_eq!(upper.len(), 3);
}

/// `after:` and `before:` bound the search, and a search can be nothing but them.
#[tokio::test]
async fn dates_bound_the_search_even_under_an_older_commit() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("from 2005");
    commit_dated(
        &repo,
        "a.txt",
        "2\n",
        "from 2024",
        "2024-06-01T12:00:00+00:00",
    );
    // A commit carrying an old date on top of a new one, as a rebase or a cherry-pick leaves.
    // `--since` would stop walking here and never reach the 2024 commit below it.
    commit_dated(
        &repo,
        "a.txt",
        "3\n",
        "old date on top",
        "2010-01-01T12:00:00+00:00",
    );
    let (runner, loc) = open(&repo).await;
    let subjects = |found: Vec<String>| -> Vec<String> {
        found
            .iter()
            .map(|o| repo.git(["log", "-1", "--format=%s", o]).trim().to_owned())
            .collect()
    };

    let after = loc
        .search_commits(&runner, "after:2020-01-01", 50)
        .await
        .unwrap();
    assert_eq!(subjects(after), ["from 2024"]);

    let before = loc
        .search_commits(&runner, "before:2008-01-01", 50)
        .await
        .unwrap();
    assert_eq!(subjects(before), ["from 2005"]);

    let between = loc
        .search_commits(&runner, "from after:2000-01-01 before:2020-01-01", 50)
        .await
        .unwrap();
    assert_eq!(subjects(between), ["from 2005"]);

    let err = loc
        .search_commits(&runner, "after:someday", 50)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "refused");
}
