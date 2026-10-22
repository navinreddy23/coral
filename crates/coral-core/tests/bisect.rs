use coral_core::bisect::Verdict;
use coral_core::process::GitRunner;
use coral_core::repo::{OpState, RepoLocation};
use coral_core::testutil::TestRepo;

async fn open(repo: &TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

/// Eight commits on main; the fifth puts `broken` into the file and it stays.
fn history() -> (TestRepo, Vec<String>) {
    let mut repo = TestRepo::new();
    let mut oids = Vec::new();
    for i in 1..=8 {
        let state = if i >= 5 { "broken" } else { "fine" };
        repo = repo
            .write("state.txt", &format!("{state} {i}\n"))
            .commit(&format!("step {i}"));
        oids.push(repo.git(["rev-parse", "HEAD"]).trim().to_owned());
    }
    (repo, oids)
}

fn checked_out_is_broken(repo: &TestRepo) -> bool {
    std::fs::read_to_string(repo.path().join("state.txt"))
        .unwrap()
        .starts_with("broken")
}

#[tokio::test]
async fn finds_the_commit_that_broke_it() {
    let (repo, oids) = history();
    let (runner, loc) = open(&repo).await;
    assert_eq!(
        loc.bisection(&runner).await.unwrap(),
        None,
        "nothing in progress yet"
    );

    // Starting is marking: the tip is bad, the first commit was fine.
    let state = loc
        .bisect_mark(&runner, Verdict::Bad, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.bad.as_deref(), Some(oids[7].as_str()));
    assert_eq!(
        state.testing, None,
        "nothing to test until a good commit is known"
    );
    assert_eq!(loc.op_state(), OpState::Bisect);

    let mut state = loc
        .bisect_mark(&runner, Verdict::Good, Some(&oids[0]))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.suspects, Some(7));

    let mut verdicts = 0;
    while state.found.is_none() {
        let testing = state.testing.clone().expect("a commit to test");
        assert_eq!(
            repo.git(["rev-parse", "HEAD"]).trim(),
            testing,
            "it is checked out"
        );
        let verdict = if checked_out_is_broken(&repo) {
            Verdict::Bad
        } else {
            Verdict::Good
        };
        state = loc
            .bisect_mark(&runner, verdict, None)
            .await
            .unwrap()
            .unwrap();
        verdicts += 1;
        assert!(
            verdicts <= 4,
            "seven suspects take three verdicts, not {verdicts}"
        );
    }
    assert_eq!(
        state.found.as_deref(),
        Some(oids[4].as_str()),
        "step 5 broke it"
    );
    // Every verdict is there, not only the newest bad commit the refs keep.
    assert_eq!(state.verdicts.len(), 2 + verdicts);
    assert_eq!(state.verdicts[0].oid, oids[7]);
    assert_eq!(state.suspects, Some(1));

    loc.bisect_reset(&runner).await.unwrap();
    assert_eq!(loc.bisection(&runner).await.unwrap(), None);
    assert_eq!(loc.op_state(), OpState::Clean);
    assert_eq!(
        repo.git(["symbolic-ref", "--short", "HEAD"]).trim(),
        "main",
        "back on the branch"
    );
}

#[tokio::test]
async fn reads_a_bisection_started_in_a_terminal() {
    let (repo, oids) = history();
    repo.git(["bisect", "start", &oids[7], &oids[0]]);
    let (runner, loc) = open(&repo).await;
    let state = loc.bisection(&runner).await.unwrap().unwrap();
    assert_eq!(state.bad.as_deref(), Some(oids[7].as_str()));
    assert_eq!(state.good, [oids[0].clone()]);
    assert_eq!(
        state.testing.as_deref(),
        Some(repo.git(["rev-parse", "HEAD"]).trim())
    );
}

#[tokio::test]
async fn reads_a_bisection_under_its_own_terms() {
    let (repo, oids) = history();
    repo.git([
        "bisect",
        "start",
        "--term-new=broken",
        "--term-old=fine",
        &oids[7],
        &oids[0],
    ]);
    let (runner, loc) = open(&repo).await;
    let state = loc.bisection(&runner).await.unwrap().unwrap();
    assert_eq!(state.terms.bad, "broken");
    assert_eq!(state.bad.as_deref(), Some(oids[7].as_str()));
    assert_eq!(state.good.len(), 1);

    // And marks with them: `git bisect bad` is refused under these terms.
    let state = loc
        .bisect_mark(&runner, Verdict::Bad, None)
        .await
        .unwrap()
        .unwrap();
    assert!(state.suspects.unwrap() < 7);
}

#[tokio::test]
async fn says_so_when_only_skipped_commits_are_left() {
    let (repo, oids) = history();
    repo.git(["bisect", "start", &oids[5], &oids[2]]);
    // Suspects are steps 4, 5 and 6; skip both that are not the bad one. git exits 2 on the
    // second, having recorded it, and that is not reported as a failure.
    let (runner, loc) = open(&repo).await;
    loc.bisect_mark(&runner, Verdict::Skip, Some(&oids[3]))
        .await
        .unwrap();
    let state = loc
        .bisect_mark(&runner, Verdict::Skip, Some(&oids[4]))
        .await
        .unwrap()
        .unwrap();
    assert!(state.only_skipped_left, "{state:?}");
    assert_eq!(state.found, None, "git cannot tell which of them it was");
    assert_eq!(state.skipped.len(), 2);
}

#[tokio::test]
async fn refuses_to_move_over_local_changes() {
    let (repo, oids) = history();
    std::fs::write(repo.path().join("state.txt"), "edited\n").unwrap();
    let (runner, loc) = open(&repo).await;
    loc.bisect_mark(&runner, Verdict::Bad, None).await.unwrap();
    let err = loc
        .bisect_mark(&runner, Verdict::Good, Some(&oids[0]))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("overwritten"), "{err}");
    assert_eq!(
        std::fs::read_to_string(repo.path().join("state.txt")).unwrap(),
        "edited\n",
        "the edit is still there"
    );
}
