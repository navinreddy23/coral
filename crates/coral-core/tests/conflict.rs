//! The conflict engine. The central claim under test is that blocks are rebuilt from the index
//! stages, so what the user sees does not depend on their `merge.conflictStyle`.

use coral_core::conflict::{Block, Blocks, Resolution, Take};
use coral_core::process::GitRunner;
use coral_core::repo::{OpState, RepoLocation};
use coral_core::status::ConflictKind;
use coral_core::testutil::TestRepo;

async fn open(repo: &TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

/// Two separated edits plus an add/add and a delete/modify, all conflicting at once.
fn conflicted() -> TestRepo {
    let r = TestRepo::new()
        .write("f.txt", "one\ntwo\nthree\nfour\nfive\n")
        .write("delmod.txt", "x\n")
        .commit("base");

    r.git(["checkout", "--quiet", "-b", "side"]);
    let r = r
        .write("f.txt", "one\nSIDE\nthree\nfour\nSIDE5\n")
        .write("addadd.txt", "side only\n");
    r.git(["rm", "--quiet", "delmod.txt"]);
    let r = r.commit("side");

    r.git(["checkout", "--quiet", "main"]);
    let r = r
        .write("f.txt", "one\nMAIN\nthree\nfour\nMAIN5\n")
        .write("delmod.txt", "y\n")
        .write("addadd.txt", "main only\n")
        .commit("main");

    // Expected to conflict, so bypass the fixture's success assertion.
    std::process::Command::new("git")
        .current_dir(r.path())
        .args(["merge", "side"])
        .output()
        .unwrap();
    r
}

#[tokio::test]
async fn lists_every_conflicted_file_with_its_kind() {
    let repo = conflicted();
    let (runner, loc) = open(&repo).await;
    let files = loc.conflicts(&runner).await.unwrap();

    let find = |p: &str| {
        files
            .iter()
            .find(|f| f.path == p)
            .unwrap_or_else(|| panic!("{p}"))
    };
    assert_eq!(files.len(), 3);
    assert_eq!(find("f.txt").kind, ConflictKind::BothModified);
    assert_eq!(find("addadd.txt").kind, ConflictKind::BothAdded);
    assert_eq!(find("delmod.txt").kind, ConflictKind::DeletedByThem);

    assert!(find("f.txt").supports_blocks());
    assert!(
        !find("delmod.txt").supports_blocks(),
        "keep or delete is the only real choice"
    );
}

/// The point of rebuilding from stages: git's default style merges adjacent conflicts into one
/// region with no base at all, and `zdiff3` changes it again. Ours must not move.
#[tokio::test]
async fn blocks_are_identical_whatever_the_user_configured() {
    let repo = conflicted();
    let (runner, loc) = open(&repo).await;

    let default = loc.conflict_blocks(&runner, "f.txt").await.unwrap();
    repo.git(["config", "merge.conflictStyle", "diff3"]);
    let diff3 = loc.conflict_blocks(&runner, "f.txt").await.unwrap();
    repo.git(["config", "merge.conflictStyle", "zdiff3"]);
    let zdiff3 = loc.conflict_blocks(&runner, "f.txt").await.unwrap();

    assert_eq!(default, diff3);
    assert_eq!(default, zdiff3);
    assert_eq!(
        default.conflict_count(),
        2,
        "two separated edits are two decisions"
    );
}

/// The worktree file git wrote has one big conflict; rebuilding gives two tight ones with a
/// real base. That difference is the whole reason for the design.
#[tokio::test]
async fn rebuilding_gives_tighter_blocks_than_the_worktree_file() {
    let repo = conflicted();
    let (runner, loc) = open(&repo).await;

    let on_disk = std::fs::read_to_string(repo.path().join("f.txt")).unwrap();
    assert_eq!(
        on_disk.matches("<<<<<<<").count(),
        1,
        "git wrote one merged region"
    );

    let blocks = loc.conflict_blocks(&runner, "f.txt").await.unwrap();
    assert_eq!(blocks.conflict_count(), 2);

    let first = blocks.blocks.iter().find_map(|b| match b {
        Block::Conflict { base, ours, theirs } => Some((base, ours, theirs)),
        Block::Common { .. } => None,
    });
    let (base, ours, theirs) = first.expect("a conflict block");
    assert_eq!(ours, &["MAIN"], "ours is the current branch's line");
    assert_eq!(theirs, &["SIDE"]);
    assert_eq!(
        base,
        &["two"],
        "and the base is present, which the default style loses"
    );
}

#[tokio::test]
async fn an_add_add_conflict_has_no_base() {
    let repo = conflicted();
    let (runner, loc) = open(&repo).await;

    let blocks = loc.conflict_blocks(&runner, "addadd.txt").await.unwrap();
    let base_empty = blocks
        .blocks
        .iter()
        .any(|b| matches!(b, Block::Conflict { base, .. } if base.is_empty()));
    assert!(base_empty, "neither side had the file before");
}

#[tokio::test]
async fn taking_one_side_writes_that_side_and_stages_it() {
    let repo = conflicted();
    let (runner, loc) = open(&repo).await;

    loc.resolve(&runner, "f.txt", &Resolution::TakeTheirs)
        .await
        .unwrap();

    assert_eq!(
        std::fs::read_to_string(repo.path().join("f.txt")).unwrap(),
        "one\nSIDE\nthree\nfour\nSIDE5\n"
    );
    let left = loc.conflicts(&runner).await.unwrap();
    assert!(
        !left.iter().any(|f| f.path == "f.txt"),
        "it is no longer conflicted"
    );
}

#[tokio::test]
async fn editing_the_output_and_saving_resolves_the_file() {
    let repo = conflicted();
    let (runner, loc) = open(&repo).await;

    let edited = "one\nMERGED BY HAND\nthree\nfour\nboth\n";
    loc.resolve(&runner, "f.txt", &Resolution::Content(edited.into()))
        .await
        .unwrap();

    assert_eq!(repo.git(["show", ":f.txt"]), edited.trim_end());
    assert!(
        !loc.conflicts(&runner)
            .await
            .unwrap()
            .iter()
            .any(|f| f.path == "f.txt")
    );
}

#[tokio::test]
async fn a_delete_modify_conflict_can_be_kept_or_deleted() {
    let repo = conflicted();
    let (runner, loc) = open(&repo).await;

    loc.resolve(&runner, "delmod.txt", &Resolution::Delete)
        .await
        .unwrap();
    assert!(!repo.path().join("delmod.txt").exists());
    assert!(
        !loc.conflicts(&runner)
            .await
            .unwrap()
            .iter()
            .any(|f| f.path == "delmod.txt")
    );
}

/// Taking a side that has no content is a mistake worth naming, not an empty file.
#[tokio::test]
async fn taking_an_absent_side_is_refused() {
    let repo = conflicted();
    let (runner, loc) = open(&repo).await;

    // "theirs" deleted delmod.txt, so there is no stage 3.
    let err = loc
        .resolve(&runner, "delmod.txt", &Resolution::TakeTheirs)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "refused");
    assert!(err.to_string().contains("delete it instead"));
}

#[tokio::test]
async fn resolving_every_file_lets_the_operation_continue() {
    let repo = conflicted();
    let (runner, loc) = open(&repo).await;

    loc.resolve(&runner, "f.txt", &Resolution::TakeOurs)
        .await
        .unwrap();
    loc.resolve(&runner, "addadd.txt", &Resolution::TakeOurs)
        .await
        .unwrap();
    loc.resolve(&runner, "delmod.txt", &Resolution::Delete)
        .await
        .unwrap();
    assert!(loc.conflicts(&runner).await.unwrap().is_empty());

    let done = loc
        .op(&runner, coral_core::ops::OpAction::Continue)
        .await
        .unwrap();
    assert!(done.completed);
    assert_eq!(loc.op_state(), OpState::Clean);
}

#[tokio::test]
async fn a_merge_labels_the_sides_by_branch_name() {
    let repo = conflicted();
    let (runner, loc) = open(&repo).await;
    let op = loc.operation(&runner).await.unwrap();

    assert_eq!(op.state, OpState::Merge);
    assert!(
        op.resumable,
        "git can be told to continue or abort this one"
    );
    assert_eq!(op.labels.ours, "main");
    assert_eq!(op.labels.theirs, "side");
    assert!(!op.labels.swapped);
}

/// A stopped cherry-pick names the commit the way git does, not by its object id.
///
/// The id was the label on a button, on a column header and in every sentence the merge tool
/// wrote: forty characters that say nothing about which commit it is.
#[tokio::test]
async fn a_cherry_pick_labels_the_incoming_side_with_the_commit() {
    let repo = TestRepo::new().write("f.txt", "base\n").commit("base");
    repo.git(["checkout", "--quiet", "-b", "side"]);
    let repo = repo.write("f.txt", "side\n").commit("a change of theirs");
    repo.git(["checkout", "--quiet", "main"]);
    let repo = repo.write("f.txt", "main\n").commit("ours");
    let (runner, loc) = open(&repo).await;

    let stopped = loc.cherry_pick(&runner, &["side"], true).await.unwrap();
    assert!(!stopped.completed);

    let op = loc.operation(&runner).await.unwrap();
    assert_eq!(op.state, OpState::CherryPick);
    assert_eq!(op.labels.ours, "main");
    assert!(
        op.labels.theirs.ends_with("(a change of theirs)"),
        "the subject is what identifies it: {}",
        op.labels.theirs
    );
    assert!(
        op.labels.theirs.len() < 30,
        "and not the whole object id: {}",
        op.labels.theirs
    );
}

/// During a rebase git replays your commits onto the target, so stage 2 is the *target* and
/// stage 3 is your own work. Reporting the raw words would tell the user the opposite.
#[tokio::test]
async fn a_rebase_reports_its_sides_as_swapped_with_progress() {
    let repo = TestRepo::new().write("f.txt", "base\n").commit("base");
    repo.git(["checkout", "--quiet", "-b", "topic"]);
    let repo = repo.write("f.txt", "topic one\n").commit("topic 1");
    let repo = repo.write("f.txt", "topic two\n").commit("topic 2");
    repo.git(["checkout", "--quiet", "main"]);
    let repo = repo.write("f.txt", "main\n").commit("main change");
    repo.git(["checkout", "--quiet", "topic"]);

    let (runner, loc) = open(&repo).await;
    let stopped = loc.rebase(&runner, "main", false).await.unwrap();
    assert!(!stopped.completed);

    let op = loc.operation(&runner).await.unwrap();
    assert_eq!(op.state, OpState::Rebase);
    assert!(
        op.labels.swapped,
        "a rebase must announce that the sides read backwards"
    );
    assert_eq!(op.head_name.as_deref(), Some("topic"));
    let progress = op.progress.expect("a rebase reports how far it has got");
    assert_eq!(progress.current, 1);
    assert_eq!(progress.total, 2);
    assert!(op.stopped_at.is_some());

    loc.op(&runner, coral_core::ops::OpAction::Abort)
        .await
        .unwrap();
}

/// Whichever side the user picks must be the side they get, even during a rebase where
/// `git checkout --ours` would give the opposite.
#[tokio::test]
async fn taking_a_side_during_a_rebase_honours_the_users_choice() {
    let repo = TestRepo::new().write("f.txt", "base\n").commit("base");
    repo.git(["checkout", "--quiet", "-b", "topic"]);
    let repo = repo.write("f.txt", "my work\n").commit("topic");
    repo.git(["checkout", "--quiet", "main"]);
    let repo = repo.write("f.txt", "the target\n").commit("main");
    repo.git(["checkout", "--quiet", "topic"]);

    let (runner, loc) = open(&repo).await;
    loc.rebase(&runner, "main", false).await.unwrap();

    // Stage 3 during a rebase is the commit being replayed: the user's own work.
    loc.resolve(&runner, "f.txt", &Resolution::TakeTheirs)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join("f.txt")).unwrap(),
        "my work\n"
    );

    loc.op(&runner, coral_core::ops::OpAction::Abort)
        .await
        .unwrap();
}

#[test]
fn parses_diff3_output_into_blocks() {
    let input =
        b"one\n<<<<<<< ours\nMAIN\n||||||| base\ntwo\n=======\nSIDE\n>>>>>>> theirs\nthree\n";
    let blocks = Blocks::parse(input).unwrap();

    assert_eq!(blocks.blocks.len(), 3);
    assert_eq!(blocks.conflict_count(), 1);
    assert_eq!(
        blocks.blocks[0],
        Block::Common {
            lines: vec!["one".into()]
        }
    );
    assert_eq!(
        blocks.blocks[1],
        Block::Conflict {
            base: vec!["two".into()],
            ours: vec!["MAIN".into()],
            theirs: vec!["SIDE".into()],
        }
    );
}

#[test]
fn renders_blocks_back_taking_either_side() {
    let input =
        b"one\n<<<<<<< ours\nMAIN\n||||||| base\ntwo\n=======\nSIDE\n>>>>>>> theirs\nthree\n";
    let blocks = Blocks::parse(input).unwrap();

    assert_eq!(blocks.render_taking(Take::Ours), "one\nMAIN\nthree\n");
    assert_eq!(blocks.render_taking(Take::Theirs), "one\nSIDE\nthree\n");
    assert_eq!(blocks.render_taking(Take::Base), "one\ntwo\nthree\n");
}

/// A line of sevens that is content, not a marker, must not be mistaken for one.
#[test]
fn a_marker_needs_exactly_seven_characters() {
    let input = b"<<<<<<<< not a marker\n========= also not\n";
    let blocks = Blocks::parse(input).unwrap();
    assert_eq!(blocks.conflict_count(), 0);
    assert_eq!(blocks.blocks.len(), 1);
}

#[test]
fn malformed_marker_sequences_are_rejected() {
    assert!(
        Blocks::parse(b"=======\n").is_err(),
        "separator with no start"
    );
    assert!(Blocks::parse(b">>>>>>> x\n").is_err(), "end with no start");
    assert!(Blocks::parse(b"<<<<<<< a\nx\n").is_err(), "unterminated");
    assert!(Blocks::parse(b"<<<<<<< a\n<<<<<<< b\n").is_err(), "nested");
}

#[test]
fn an_empty_file_has_no_blocks() {
    assert!(Blocks::parse(b"").unwrap().blocks.is_empty());
    assert!(Blocks::parse(b"\n").unwrap().blocks.is_empty());
}

/// Merging a release tag must name it. Kernel releases have no branch pointing at them, so
/// excluding tags from ref resolution left the user staring at an abbreviated object id.
#[tokio::test]
async fn a_tag_being_merged_is_named_not_abbreviated() {
    let repo = TestRepo::new().write("f.txt", "base\n").commit("base");
    repo.git(["checkout", "--quiet", "-b", "side"]);
    let repo = repo.write("f.txt", "side\n").commit("side");
    repo.git(["tag", "v2.0"]);
    repo.git(["checkout", "--quiet", "main"]);
    let repo = repo.write("f.txt", "main\n").commit("main");
    // Leave only the tag, as a kernel release does.
    repo.git(["branch", "-D", "side"]);

    let (runner, loc) = open(&repo).await;
    loc.merge(&runner, "v2.0", coral_core::ops::MergeMode::Auto, None)
        .await
        .unwrap();

    let op = loc.operation(&runner).await.unwrap();
    assert_eq!(
        op.labels.theirs, "v2.0",
        "the bare tag name, not tags/v2.0 or an object id"
    );
    assert_eq!(op.labels.ours, "main");
}

/// A conflicted index with nothing in the git dir to mark it.
///
/// `cherry-pick --no-commit` and `merge --no-commit` both leave one: the files are unmerged and
/// there is no `CHERRY_PICK_HEAD` or `MERGE_HEAD` to find. Reading the git dir alone said nothing
/// was happening, and the window offered no way to resolve them.
#[test]
fn an_unmerged_index_is_an_operation_even_with_no_marker_file() {
    let repo = TestRepo::new()
        .write("dummy.txt", "dummy file\n")
        .commit("first");
    repo.git(["checkout", "--quiet", "-b", "theirs"]);
    let repo = repo.write("dummy.txt", "dummy2 file\n").commit("theirs");
    repo.git(["checkout", "--quiet", "main"]);
    let repo = repo.write("dummy.txt", "dummy1 file\n").commit("ours");

    // git exits non-zero for the conflict; the fixture's own runner would panic on that.
    let _ = repo
        .command(["cherry-pick", "--no-commit", "theirs"])
        .output()
        .expect("spawn git");

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();

        // Nothing in the git dir says so.
        assert_eq!(loc.op_state(), OpState::Clean);

        let op = loc.operation(&runner).await.unwrap();
        assert_eq!(
            op.state,
            OpState::Merge,
            "the window would show no merge tool"
        );
        // Called a merge, but there is nothing to continue: `git merge --continue` answers
        // that no merge is in progress, so the window must not offer the button.
        assert!(!op.resumable);
        assert_eq!(loc.conflicts(&runner).await.unwrap().len(), 1);
    });
}
