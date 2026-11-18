use coral_core::grep::{GrepPattern, GrepQuery, MOST};
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

async fn open(repo: &TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

fn query(pattern: &str, how: GrepPattern) -> GrepQuery {
    GrepQuery {
        pattern: pattern.to_owned(),
        how,
        ignore_case: false,
        paths: Vec::new(),
    }
}

#[tokio::test]
async fn finds_each_line_at_a_revision_with_its_path_line_and_column() {
    let repo = TestRepo::new()
        .write("src/a.c", "int count;\nvoid f(void) { count++; }\n")
        .write("b with space.txt", "no match\nthe count is 3\n")
        .commit("first");
    let first = repo.git(["rev-parse", "HEAD"]).trim().to_owned();
    // Gone from the working copy and from HEAD: the search is of the revision asked about.
    let repo = repo.write("src/a.c", "int total;\n").commit("second");

    let (runner, loc) = open(&repo).await;
    let found = loc
        .grep(&runner, &first, &query("count", GrepPattern::Fixed))
        .await
        .unwrap();
    let hits: Vec<(String, u32, u32, String)> = found
        .matches
        .iter()
        .map(|m| (m.path.to_string(), m.line, m.column, m.text.to_string()))
        .collect();
    assert_eq!(
        hits,
        vec![
            (
                "b with space.txt".to_owned(),
                2,
                5,
                "the count is 3".to_owned()
            ),
            ("src/a.c".to_owned(), 1, 5, "int count;".to_owned()),
            (
                "src/a.c".to_owned(),
                2,
                16,
                "void f(void) { count++; }".to_owned()
            ),
        ]
    );
    assert!(!found.more);

    let now = loc
        .grep(&runner, "HEAD", &query("count", GrepPattern::Fixed))
        .await
        .unwrap();
    assert_eq!(now.matches.len(), 1, "only the text file still says it");
}

#[tokio::test]
async fn takes_a_pattern_as_text_or_as_an_expression_and_can_ignore_case() {
    let repo = TestRepo::new()
        .write("a.txt", "Alpha(1)\nalpha\nbeta\n")
        .commit("first");
    let (runner, loc) = open(&repo).await;

    // Taken as text, the parentheses are only parentheses.
    let fixed = loc
        .grep(&runner, "HEAD", &query("a(1", GrepPattern::Fixed))
        .await
        .unwrap();
    assert_eq!(fixed.matches.len(), 1);
    let regex = loc
        .grep(
            &runner,
            "HEAD",
            &query("^(alpha|beta)$", GrepPattern::Regex),
        )
        .await
        .unwrap();
    assert_eq!(regex.matches.len(), 2);
    let any_case = GrepQuery {
        ignore_case: true,
        ..query("ALPHA", GrepPattern::Fixed)
    };
    assert_eq!(
        loc.grep(&runner, "HEAD", &any_case)
            .await
            .unwrap()
            .matches
            .len(),
        2
    );

    // An expression git cannot read is its refusal, not an empty answer.
    let broken = loc
        .grep(&runner, "HEAD", &query("(", GrepPattern::Regex))
        .await;
    assert!(broken.is_err());
}

#[tokio::test]
async fn looks_only_where_it_is_told_and_says_nothing_found_as_an_empty_answer() {
    let repo = TestRepo::new()
        .write("src/a.c", "needle\n")
        .write("docs/a.md", "needle\n")
        .commit("first");
    let (runner, loc) = open(&repo).await;
    let in_src = GrepQuery {
        paths: vec!["src".to_owned()],
        ..query("needle", GrepPattern::Fixed)
    };
    let found = loc.grep(&runner, "HEAD", &in_src).await.unwrap();
    assert_eq!(found.matches.len(), 1);
    assert_eq!(found.matches[0].path, "src/a.c");

    let none = loc
        .grep(&runner, "HEAD", &query("haystack", GrepPattern::Fixed))
        .await
        .unwrap();
    assert_eq!(none.matches, []);
}

#[tokio::test]
async fn stops_at_the_most_it_will_hold_and_says_there_was_more() {
    let lines = "x\n".repeat(MOST + 10);
    let repo = TestRepo::new().write("many.txt", &lines).commit("first");
    let (runner, loc) = open(&repo).await;
    let found = loc
        .grep(&runner, "HEAD", &query("x", GrepPattern::Fixed))
        .await
        .unwrap();
    assert_eq!(found.matches.len(), MOST);
    assert!(found.more);
}
