//! Interactive rebase driven by a prepared todo list, with no editor on the path.

use coral_core::ops::OpAction;
use coral_core::patch::PatchLanding;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::sequence::{Step, Todo};
use coral_core::testutil::TestRepo;

/// Three commits on top of a base, oldest first: a, b, c.
fn stack() -> TestRepo {
    TestRepo::new()
        .write("base.txt", "base\n")
        .commit("base")
        .write("a.txt", "a\n")
        .commit("commit a")
        .write("b.txt", "b\n")
        .commit("commit b")
        .write("c.txt", "c\n")
        .commit("commit c")
}

fn coral_binary() -> std::path::PathBuf {
    // The test binary sits beside the CLI in the same profile directory.
    let mut at = std::env::current_exe().expect("test binary path");
    at.pop();
    if at.ends_with("deps") {
        at.pop();
    }
    let binary = at.join(if cfg!(windows) { "coral.exe" } else { "coral" });
    assert!(
        binary.exists(),
        "the CLI must be built for these: {}",
        binary.display()
    );
    binary
}

fn summaries(repo: &TestRepo) -> Vec<String> {
    repo.git(["log", "--format=%s"])
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_todo_lists_the_range_oldest_first() {
    let repo = stack();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let todo = loc.rebase_todo(&runner, "HEAD~3").await.unwrap();

        // Replay order, which is the reverse of how the graph lists them.
        let order: Vec<_> = todo.items.iter().map(|i| i.summary.to_string()).collect();
        assert_eq!(order, ["commit a", "commit b", "commit c"]);
        assert!(todo.items.iter().all(|i| i.step == Step::Pick));
        assert!(todo.items.iter().all(|i| i.oid.len() == 40));
    });
}

#[test]
fn a_todo_across_a_merge_is_refused_rather_than_offered_flattened() {
    // The list is built with --no-merges, so a range holding one comes back a commit short and
    // says nothing about it. Starting that rebase rewrites the history into a straight line.
    // `rewrite_commit` already refused this; the picker and the command line asked for the same
    // list and were handed it.
    let repo = TestRepo::new()
        .write("base.txt", "base\n")
        .commit("base")
        .write("a.txt", "a\n")
        .commit("commit a");
    repo.git(["checkout", "--quiet", "-b", "side", "HEAD~1"]);
    let repo = repo.write("side.txt", "side\n").commit("commit side");
    repo.git(["checkout", "--quiet", "main"]);
    repo.git(["merge", "--quiet", "--no-ff", "-m", "merge side", "side"]);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        // HEAD~2 is the base: the range holds commit a, commit side and the merge, and the
        // list would come back with the first two.
        let error = loc.rebase_todo(&runner, "HEAD~2").await.unwrap_err();
        assert_eq!(error.code(), "refused", "{error}");
        assert!(error.to_string().contains("merge"), "{error}");
    });
}

#[test]
fn dropping_a_commit_removes_it_and_its_file() {
    let repo = stack();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let mut todo = loc.rebase_todo(&runner, "HEAD~3").await.unwrap();
        todo.items[1].step = Step::Drop;

        let out = loc
            .rebase_interactive(&runner, "HEAD~3", &todo, &coral_binary())
            .await
            .unwrap();
        assert!(
            out.completed,
            "no conflict: the three touch different files. git said: {}",
            out.message
        );
    });

    assert_eq!(summaries(&repo), ["commit c", "commit a", "base"]);
    assert!(!repo.path().join("b.txt").exists());
}

#[test]
fn reordering_replays_in_the_new_order() {
    let repo = stack();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let mut todo = loc.rebase_todo(&runner, "HEAD~3").await.unwrap();
        todo.reorder(2, 0).unwrap();

        let out = loc
            .rebase_interactive(&runner, "HEAD~3", &todo, &coral_binary())
            .await
            .unwrap();
        assert!(out.completed);
    });

    assert_eq!(
        summaries(&repo),
        ["commit b", "commit a", "commit c", "base"]
    );
}

#[test]
fn a_fixup_folds_a_commit_into_the_one_before_it() {
    let repo = stack();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let mut todo = loc.rebase_todo(&runner, "HEAD~3").await.unwrap();
        todo.items[2].step = Step::Fixup;

        // A fixup opens an editor on the combined message that nobody is there to answer; the
        // stubbed core.editor is what keeps this from hanging.
        let out = loc
            .rebase_interactive(&runner, "HEAD~3", &todo, &coral_binary())
            .await
            .unwrap();
        assert!(out.completed);
    });

    assert_eq!(summaries(&repo), ["commit b", "commit a", "base"]);
    // The folded commit's work is still there, only its message is gone.
    assert!(repo.path().join("c.txt").exists());
}

#[test]
fn a_squash_keeps_both_messages() {
    let repo = stack();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let mut todo = loc.rebase_todo(&runner, "HEAD~3").await.unwrap();
        todo.items[2].step = Step::Squash;
        let out = loc
            .rebase_interactive(&runner, "HEAD~3", &todo, &coral_binary())
            .await
            .unwrap();
        assert!(out.completed);
    });

    let body = repo.git(["log", "-1", "--format=%B"]);
    assert!(body.contains("commit b"), "kept the first message: {body}");
    assert!(body.contains("commit c"), "kept the second: {body}");
}

#[test]
fn a_list_that_starts_with_a_squash_is_refused_before_anything_moves() {
    // git would fail partway through, leaving a rebase to abort; refusing first leaves the
    // branch exactly where it was.
    let repo = stack();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let before = repo.git(["rev-parse", "HEAD"]);
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let mut todo = loc.rebase_todo(&runner, "HEAD~3").await.unwrap();
        todo.items[0].step = Step::Squash;
        assert!(
            loc.rebase_interactive(&runner, "HEAD~3", &todo, &coral_binary())
                .await
                .is_err()
        );
    });
    assert_eq!(repo.git(["rev-parse", "HEAD"]), before);
    assert_eq!(repo.git(["status", "--porcelain"]).trim(), "");
}

#[test]
fn the_todo_file_is_not_left_behind() {
    let repo = stack();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let todo: Todo = loc.rebase_todo(&runner, "HEAD~3").await.unwrap();
        loc.rebase_interactive(&runner, "HEAD~3", &todo, &coral_binary())
            .await
            .unwrap();
        // Anything reading the git dir would take a leftover as a rebase in progress.
        assert!(!loc.git_path("coral-rebase-todo").exists());
    });
}

#[test]
fn a_reword_replaces_only_that_commit_message() {
    let repo = stack();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let mut todo = loc.rebase_todo(&runner, "HEAD~3").await.unwrap();
        todo.items[1].step = Step::Reword;
        todo.items[1].message = Some("commit b, said better".to_owned());

        let out = loc
            .rebase_interactive(&runner, "HEAD~3", &todo, &coral_binary())
            .await
            .unwrap();
        assert!(
            out.completed,
            "it must not be left stopped: {}",
            out.message
        );
    });

    assert_eq!(
        summaries(&repo),
        ["commit c", "commit b, said better", "commit a", "base"]
    );
    assert_eq!(repo.git(["status", "--porcelain"]).trim(), "");
}

#[test]
fn several_rewords_each_get_their_own_message() {
    // git's own `reword` opens an editor with no way to say which commit it is asking about.
    // Stopping on each and amending is what makes more than one of them possible at all.
    let repo = stack();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let mut todo = loc.rebase_todo(&runner, "HEAD~3").await.unwrap();
        for (i, text) in [(0, "first, renamed"), (2, "third, renamed")] {
            todo.items[i].step = Step::Reword;
            todo.items[i].message = Some(text.to_owned());
        }
        let out = loc
            .rebase_interactive(&runner, "HEAD~3", &todo, &coral_binary())
            .await
            .unwrap();
        assert!(out.completed, "{}", out.message);
    });

    assert_eq!(
        summaries(&repo),
        ["third, renamed", "commit b", "first, renamed", "base"]
    );
}

#[test]
fn a_reword_survives_the_commits_above_it_being_rewritten() {
    // The message has to reach the right commit after everything above it has a new object
    // id, which is why the stop is matched on the id git records rather than on position.
    let repo = stack();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let mut todo = loc.rebase_todo(&runner, "HEAD~3").await.unwrap();
        todo.reorder(2, 0).unwrap();
        todo.items[2].step = Step::Reword;
        let renamed = todo.items[2].summary.to_string();
        todo.items[2].message = Some(format!("{renamed}, renamed"));

        let out = loc
            .rebase_interactive(&runner, "HEAD~3", &todo, &coral_binary())
            .await
            .unwrap();
        assert!(out.completed, "{}", out.message);
    });

    assert_eq!(
        summaries(&repo),
        ["commit b, renamed", "commit a", "commit c", "base"]
    );
}

#[test]
fn a_reword_with_no_message_leaves_the_commit_alone() {
    // Better than opening an editor nobody can answer, which would hang the rebase.
    let repo = stack();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let mut todo = loc.rebase_todo(&runner, "HEAD~3").await.unwrap();
        todo.items[1].step = Step::Reword;
        let out = loc
            .rebase_interactive(&runner, "HEAD~3", &todo, &coral_binary())
            .await
            .unwrap();
        assert!(out.completed, "{}", out.message);
    });
    assert_eq!(
        summaries(&repo),
        ["commit c", "commit b", "commit a", "base"]
    );
}

#[test]
fn an_edit_the_user_asked_for_still_stops() {
    // The rebase drives itself through its own rewords and hands back on anything else.
    let repo = stack();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let mut todo = loc.rebase_todo(&runner, "HEAD~3").await.unwrap();
        todo.items[0].step = Step::Reword;
        todo.items[0].message = Some("first, renamed".to_owned());
        todo.items[2].step = Step::Edit;

        let out = loc
            .rebase_interactive(&runner, "HEAD~3", &todo, &coral_binary())
            .await
            .unwrap();
        assert!(!out.completed, "it must stop where the user asked it to");
    });

    // The reword before it went through; the rebase is waiting at the edit.
    assert!(repo.git(["log", "--format=%s"]).contains("first, renamed"));
    repo.git(["rebase", "--abort"]);
}

/// A patch file made by one repository, applied to another.
///
/// The whole point of the format: the commit crosses a machine with its author and message
/// intact, and lands as a commit rather than as a pile of working-tree changes.
#[tokio::test(flavor = "multi_thread")]
async fn a_patch_file_applies_as_a_commit_with_its_author_kept() {
    let runner = GitRunner::discover().await.unwrap();
    let source = TestRepo::new().write("a.txt", "one\n").commit("base");
    let source = source
        .write("a.txt", "two\n")
        .commit("the change worth sending");
    let out = tempfile::tempdir().unwrap();
    let loc = RepoLocation::discover(&runner, source.path())
        .await
        .unwrap();
    let files = loc.format_patch(&runner, "HEAD", out.path()).await.unwrap();
    assert_eq!(files.len(), 1);

    let target = TestRepo::new().write("a.txt", "one\n").commit("base");
    let there = RepoLocation::discover(&runner, target.path())
        .await
        .unwrap();
    let outcome = there
        .apply_patches(&runner, &files, PatchLanding::Commit)
        .await
        .unwrap();

    assert!(outcome.completed, "a clean patch should not stop");
    assert_eq!(
        target.git(["log", "-1", "--format=%s"]).trim(),
        "the change worth sending"
    );
    assert_eq!(
        std::fs::read_to_string(target.path().join("a.txt")).unwrap(),
        "two\n"
    );
    assert_eq!(target.git(["status", "--short"]).trim(), "");
}

/// The other landing: the change is there to look at, and nothing has been recorded.
#[tokio::test(flavor = "multi_thread")]
async fn a_patch_can_be_left_in_the_working_tree_instead() {
    let runner = GitRunner::discover().await.unwrap();
    let source = TestRepo::new().write("a.txt", "one\n").commit("base");
    let source = source
        .write("a.txt", "two\n")
        .commit("a change to look at first");
    let out = tempfile::tempdir().unwrap();
    let loc = RepoLocation::discover(&runner, source.path())
        .await
        .unwrap();
    let files = loc.format_patch(&runner, "HEAD", out.path()).await.unwrap();

    let target = TestRepo::new().write("a.txt", "one\n").commit("base");
    let there = RepoLocation::discover(&runner, target.path())
        .await
        .unwrap();
    there
        .apply_patches(&runner, &files, PatchLanding::WorkingTree)
        .await
        .unwrap();

    assert_eq!(
        std::fs::read_to_string(target.path().join("a.txt")).unwrap(),
        "two\n"
    );
    assert_eq!(target.git(["log", "-1", "--format=%s"]).trim(), "base");
    assert!(
        !target.git(["status", "--short"]).trim().is_empty(),
        "the change is uncommitted"
    );
}

/// A patch that cannot be applied stops in the conflict tool rather than failing, and is
/// continued as `git am` — not as the rebase its state files look like.
#[tokio::test(flavor = "multi_thread")]
async fn a_conflicting_patch_stops_and_is_settled_as_an_am() {
    let runner = GitRunner::discover().await.unwrap();
    let source = TestRepo::new().write("a.txt", "one\n").commit("base");
    let source = source
        .write("a.txt", "from the patch\n")
        .commit("their change");
    let out = tempfile::tempdir().unwrap();
    let loc = RepoLocation::discover(&runner, source.path())
        .await
        .unwrap();
    let files = loc.format_patch(&runner, "HEAD", out.path()).await.unwrap();

    let target = TestRepo::new().write("a.txt", "one\n").commit("base");
    let target = target.write("a.txt", "ours instead\n").commit("our change");
    let there = RepoLocation::discover(&runner, target.path())
        .await
        .unwrap();
    let outcome = there
        .apply_patches(&runner, &files, PatchLanding::Commit)
        .await
        .unwrap();

    assert!(!outcome.completed, "a conflicting patch has to stop");
    assert!(
        there.applying_patches(),
        "it is an am, whatever the directory is called"
    );
    assert_eq!(there.op_state(), coral_core::repo::OpState::Rebase);

    // The window offers Abort on that stop; it has to reach `git am --abort`.
    there.op(&runner, OpAction::Abort).await.unwrap();
    assert_eq!(there.op_state(), coral_core::repo::OpState::Clean);
    assert_eq!(
        target.git(["log", "-1", "--format=%s"]).trim(),
        "our change"
    );
}

/// Applying the same series twice is the ordinary result of a re-sent mail, and must not stop
/// halfway through asking a question the window has no way to answer.
#[tokio::test(flavor = "multi_thread")]
async fn applying_a_patch_that_has_already_landed_is_not_a_stop() {
    let runner = GitRunner::discover().await.unwrap();
    let source = TestRepo::new().write("a.txt", "one\n").commit("base");
    let source = source.write("a.txt", "two\n").commit("the change");
    let out = tempfile::tempdir().unwrap();
    let loc = RepoLocation::discover(&runner, source.path())
        .await
        .unwrap();
    let files = loc.format_patch(&runner, "HEAD", out.path()).await.unwrap();

    let target = TestRepo::new().write("a.txt", "one\n").commit("base");
    let there = RepoLocation::discover(&runner, target.path())
        .await
        .unwrap();
    there
        .apply_patches(&runner, &files, PatchLanding::Commit)
        .await
        .unwrap();
    let again = there
        .apply_patches(&runner, &files, PatchLanding::Commit)
        .await
        .unwrap();

    assert!(again.completed, "the second run should not stop");
    assert_eq!(there.op_state(), coral_core::repo::OpState::Clean);
}

#[tokio::test(flavor = "multi_thread")]
async fn naming_no_patch_file_is_refused_rather_than_run() {
    let runner = GitRunner::discover().await.unwrap();
    let repo = TestRepo::new().write("a.txt", "one\n").commit("base");
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    let refused = loc.apply_patches(&runner, &[], PatchLanding::Commit).await;
    assert!(matches!(
        refused,
        Err(coral_core::CoralError::Refused { .. })
    ));
}

/// Two commits picked in the window means "what lies between them", which is `from..to`:
/// exclusive at the older end, one patch per commit after it.
#[tokio::test(flavor = "multi_thread")]
async fn a_range_exports_one_patch_per_commit_after_the_older_end() {
    let runner = GitRunner::discover().await.unwrap();
    let repo = stack();
    let out = tempfile::tempdir().unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();

    let files = loc
        .format_patch_range(&runner, "HEAD~3", "HEAD", out.path())
        .await
        .unwrap();

    assert_eq!(files.len(), 3, "three commits after the base");
    let names: Vec<String> = files
        .iter()
        .map(|f| f.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(
        names[0].starts_with("0001-"),
        "numbered so a series keeps its order: {names:?}"
    );
    assert!(names[2].starts_with("0003-"), "{names:?}");

    // And the range is exclusive at the older end: the base is not in it.
    assert!(
        !names.iter().any(|n| n.contains("base")),
        "the older end is excluded: {names:?}"
    );
}

/// The whole exchange, both halves, against a second repository.
#[tokio::test(flavor = "multi_thread")]
async fn a_range_written_out_applies_as_the_same_commits_elsewhere() {
    let runner = GitRunner::discover().await.unwrap();
    let source = stack();
    let out = tempfile::tempdir().unwrap();
    let loc = RepoLocation::discover(&runner, source.path())
        .await
        .unwrap();
    let files = loc
        .format_patch_range(&runner, "HEAD~3", "HEAD", out.path())
        .await
        .unwrap();
    let subjects = source.git(["log", "--format=%s", "-3", "--reverse"]);

    // A repository holding only the base those patches were made against.
    let target = TestRepo::new().write("base.txt", "base\n").commit("base");
    let there = RepoLocation::discover(&runner, target.path())
        .await
        .unwrap();
    let outcome = there
        .apply_patches(&runner, &files, PatchLanding::Commit)
        .await
        .unwrap();

    assert!(outcome.completed);
    assert_eq!(
        target.git(["log", "--format=%s", "-3", "--reverse"]),
        subjects
    );
    assert_eq!(target.git(["status", "--short"]).trim(), "");
}

/// A reword on a commit that also conflicts.
///
/// The reword is replayed as an `edit` and the message written on when the rebase stops there.
/// A conflict stops it first and hands the rebase to whatever settles conflicts, so the
/// message has to outlive the call that started the rebase — held in this function it was
/// dropped, and the commit kept the message the user had just replaced.
#[test]
fn a_reword_survives_a_conflict_on_the_same_commit() {
    let repo = TestRepo::new()
        .write("f.txt", "base\n")
        .commit("base")
        .write("f.txt", "from the topic\n")
        .commit("the commit being reworded");
    let onto = repo.git(["rev-parse", "HEAD~1"]);
    repo.git(["branch", "--quiet", "topic"]);
    repo.git(["checkout", "--quiet", "HEAD~1"]);
    let repo = repo.write("f.txt", "from elsewhere\n").commit("elsewhere");
    let target = repo.git(["rev-parse", "HEAD"]);
    repo.git(["checkout", "--quiet", "topic"]);
    let _ = onto;

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let mut todo = loc.rebase_todo(&runner, &target).await.unwrap();
        assert_eq!(todo.items.len(), 1, "one commit to replay");
        todo.items[0].step = Step::Reword;
        todo.items[0].message = Some("the message the user typed".to_owned());

        let stopped = loc
            .rebase_interactive(&runner, &target, &todo, &coral_binary())
            .await
            .unwrap();
        assert!(!stopped.completed, "it stops on the conflict");
        assert_eq!(stopped.conflicts, vec!["f.txt".to_owned()]);

        // Settle it the way the merge tool does, then continue through the same path it uses.
        std::fs::write(repo.path().join("f.txt"), "settled\n").unwrap();
        repo.git(["add", "f.txt"]);
        let done = loc.op(&runner, OpAction::Continue).await.unwrap();
        assert!(done.completed, "the rebase finishes: {}", done.message);
    });

    assert_eq!(summaries(&repo)[0], "the message the user typed");
    assert!(
        !repo
            .path()
            .join(".git")
            .join("coral-rebase-rewords")
            .exists(),
        "and nothing is left behind for the next rebase to pick up"
    );
}

/// Abandoning one throws the messages away with it.
#[test]
fn abandoning_a_rebase_forgets_the_messages_it_owed() {
    let repo = TestRepo::new()
        .write("f.txt", "base\n")
        .commit("base")
        .write("f.txt", "from the topic\n")
        .commit("the commit being reworded");
    repo.git(["branch", "--quiet", "topic"]);
    repo.git(["checkout", "--quiet", "HEAD~1"]);
    let repo = repo.write("f.txt", "from elsewhere\n").commit("elsewhere");
    let target = repo.git(["rev-parse", "HEAD"]);
    repo.git(["checkout", "--quiet", "topic"]);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let mut todo = loc.rebase_todo(&runner, &target).await.unwrap();
        todo.items[0].step = Step::Reword;
        todo.items[0].message = Some("never applied".to_owned());
        let stopped = loc
            .rebase_interactive(&runner, &target, &todo, &coral_binary())
            .await
            .unwrap();
        assert!(!stopped.completed);

        loc.op(&runner, OpAction::Abort).await.unwrap();
    });

    assert_eq!(summaries(&repo)[0], "the commit being reworded");
    assert!(
        !repo
            .path()
            .join(".git")
            .join("coral-rebase-rewords")
            .exists()
    );
}
