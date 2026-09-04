//! Every shape `git diff` emits, checked against a live repository rather than only against
//! captured text, so the parser cannot drift from what git actually produces.

use std::fmt::Write as _;

use coral_core::diff::{Context, FileChange, LineKind};
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

/// One repository carrying every case at once: add, delete, modify, rename, mode change,
/// binary, no-trailing-newline, and a path containing a space.
fn every_shape() -> TestRepo {
    let r = TestRepo::new()
        .write("mod.txt", "one\ntwo\nthree\n")
        .write("del.txt", "gone\n")
        .write("ren.txt", "old name\n")
        .write("mode.txt", "x\n")
        .write("nonl.txt", "no newline at end")
        .write("with space.txt", "spaced\n")
        .commit("base");

    let r = r
        .write("mod.txt", "one\nTWO\nthree\nfour\n")
        .write("nonl.txt", "no newline CHANGED")
        .write("add.txt", "brand new\n")
        .write("with space.txt", "spaced and changed\n");
    r.git(["rm", "--quiet", "del.txt"]);
    r.git(["mv", "ren.txt", "ren-new.txt"]);
    std::fs::set_permissions(
        r.path().join("mode.txt"),
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .unwrap();
    r.git(["add", "--all"]);
    r
}

async fn staged(repo: &TestRepo) -> Vec<coral_core::diff::FileDiff> {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    loc.diff(&runner, true, &[], Context::Hunks).await.unwrap()
}

#[tokio::test]
async fn classifies_every_kind_of_change() {
    let repo = every_shape();
    let files = staged(&repo).await;
    let find = |name: &str| {
        files
            .iter()
            .find(|f| f.path == name)
            .unwrap_or_else(|| panic!("{name} missing"))
    };

    assert_eq!(find("add.txt").change, FileChange::Added);
    assert_eq!(find("del.txt").change, FileChange::Deleted);
    assert_eq!(find("mod.txt").change, FileChange::Modified);
    assert_eq!(find("ren-new.txt").change, FileChange::Renamed);
    assert_eq!(
        find("ren-new.txt")
            .old_path
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some("ren.txt")
    );
}

/// `diff --git a/with space.txt b/with space.txt` cannot be split reliably, and git does not
/// quote for a plain space. Paths therefore come from the NUL-delimited numstat.
#[tokio::test]
async fn a_path_containing_a_space_survives() {
    let repo = every_shape();
    let files = staged(&repo).await;

    let f = files
        .iter()
        .find(|f| f.path == "with space.txt")
        .expect("spaced path missing");
    assert_eq!(f.change, FileChange::Modified);
    assert!(
        !f.hunks.is_empty(),
        "the spaced file should still have hunks"
    );
}

#[tokio::test]
async fn line_numbers_and_kinds_track_through_a_hunk() {
    let repo = every_shape();
    let files = staged(&repo).await;
    let f = files.iter().find(|f| f.path == "mod.txt").unwrap();

    assert_eq!(f.added, Some(2));
    assert_eq!(f.removed, Some(1));
    let h = &f.hunks[0];
    assert_eq!((h.old_start, h.new_start), (1, 1));

    let kinds: Vec<_> = h.lines.iter().map(|l| l.kind).collect();
    assert_eq!(
        kinds,
        vec![
            LineKind::Context,
            LineKind::Remove,
            LineKind::Add,
            LineKind::Context,
            LineKind::Add
        ]
    );
    // Context advances both sides; an add advances only the new side.
    assert_eq!((h.lines[0].old_no, h.lines[0].new_no), (Some(1), Some(1)));
    assert_eq!((h.lines[1].old_no, h.lines[1].new_no), (Some(2), None));
    assert_eq!((h.lines[2].old_no, h.lines[2].new_no), (None, Some(2)));
    assert_eq!((h.lines[4].old_no, h.lines[4].new_no), (None, Some(4)));
}

/// `\ No newline at end of file` annotates the line above rather than being a line itself.
#[tokio::test]
async fn a_missing_trailing_newline_annotates_its_line() {
    let repo = every_shape();
    let files = staged(&repo).await;
    let f = files.iter().find(|f| f.path == "nonl.txt").unwrap();

    let h = &f.hunks[0];
    assert!(h.lines.iter().all(|l| l.kind != LineKind::Context));
    assert!(
        h.lines.iter().all(|l| l.no_newline),
        "both sides lack a trailing newline, and neither backslash line became a line"
    );
    assert!(h.lines.iter().all(|l| !l.text.starts_with(b"\\")));
}

#[tokio::test]
async fn a_binary_file_reports_no_counts_and_no_hunks() {
    let repo = TestRepo::new();
    std::fs::write(repo.path().join("bin.dat"), [0_u8, 1, 2, 0, 255, 3]).unwrap();
    let repo = repo.commit("base");
    std::fs::write(repo.path().join("bin.dat"), [9_u8, 8, 0, 7, 0, 6]).unwrap();
    repo.git(["add", "--all"]);

    let files = staged(&repo).await;
    let f = &files[0];
    assert!(f.binary);
    assert_eq!((f.added, f.removed), (None, None));
    assert!(f.hunks.is_empty());
}

/// A mode change carries no hunks; the file must still be reported.
#[tokio::test]
async fn a_mode_only_change_is_reported_without_hunks() {
    let repo = every_shape();
    let files = staged(&repo).await;
    let f = files.iter().find(|f| f.path == "mode.txt").unwrap();

    assert_eq!((f.added, f.removed), (Some(0), Some(0)));
    assert!(f.hunks.is_empty());
}

#[tokio::test]
async fn hunks_land_on_the_right_files_when_several_change_at_once() {
    let repo = every_shape();
    let files = staged(&repo).await;

    for f in &files {
        for h in &f.hunks {
            for l in &h.lines {
                assert!(
                    !l.text.starts_with(b"diff --git"),
                    "{}: a section boundary leaked into a hunk",
                    f.path
                );
            }
        }
    }
    let modded = files.iter().find(|f| f.path == "mod.txt").unwrap();
    assert!(modded.hunks[0].lines.iter().any(|l| l.text == "TWO"));
    let added = files.iter().find(|f| f.path == "add.txt").unwrap();
    assert!(added.hunks[0].lines.iter().any(|l| l.text == "brand new"));
}

#[tokio::test]
async fn a_clean_repository_diffs_to_nothing() {
    let repo = TestRepo::new().write("a.txt", "a\n").commit("base");
    assert!(staged(&repo).await.is_empty());
}

/// Regression: splitting a patch on newlines leaves a trailing empty element, which is
/// indistinguishable from a context line whose single space was stripped. Reading until the
/// prefix stopped matching therefore appended a phantom context line to the *last* file of
/// every patch — invisible until a generated patch was fed back to `git apply`, which
/// rejected it because the hunk claimed one line more than the file has.
#[tokio::test]
async fn the_last_file_in_a_patch_gets_no_phantom_trailing_line() {
    let repo = TestRepo::new()
        .write("only.txt", "keep\ndrop me\ntail\n")
        .commit("base");
    let repo = repo.write("only.txt", "keep\ntail\n");
    repo.git(["add", "--all"]);

    let files = staged(&repo).await;
    let hunk = &files[0].hunks[0];

    let old: u32 = hunk
        .lines
        .iter()
        .filter(|l| l.kind != coral_core::diff::LineKind::Add)
        .count()
        .try_into()
        .unwrap();
    let new: u32 = hunk
        .lines
        .iter()
        .filter(|l| l.kind != coral_core::diff::LineKind::Remove)
        .count()
        .try_into()
        .unwrap();

    assert_eq!(
        old, hunk.old_lines,
        "parsed old-side lines must match the header"
    );
    assert_eq!(
        new, hunk.new_lines,
        "parsed new-side lines must match the header"
    );
    assert!(
        hunk.lines.iter().all(|l| !l.text.is_empty()),
        "no phantom empty line"
    );
}

/// Every hunk of every file must agree with its own header, which is what makes a generated
/// patch applicable.
#[tokio::test]
async fn every_hunk_agrees_with_its_header() {
    let repo = every_shape();
    for f in staged(&repo).await {
        for h in &f.hunks {
            let old = h.lines.iter().filter(|l| l.kind != LineKind::Add).count();
            let new = h
                .lines
                .iter()
                .filter(|l| l.kind != LineKind::Remove)
                .count();
            assert_eq!(u32::try_from(old).unwrap(), h.old_lines, "{}: old", f.path);
            assert_eq!(u32::try_from(new).unwrap(), h.new_lines, "{}: new", f.path);
        }
    }
}

#[test]
fn a_commit_diff_carries_hunks_for_one_file() {
    let repo = TestRepo::new()
        .write("a.txt", "one\ntwo\nthree\n")
        .write("b.txt", "keep\n")
        .commit("first")
        .write("a.txt", "one\nTWO\nthree\n")
        .commit("second");

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();

        let files = loc
            .commit_diff(&runner, "HEAD", &["a.txt"], Context::Hunks)
            .await
            .unwrap();
        assert_eq!(files.len(), 1, "narrowed to the one path asked for");
        let f = &files[0];
        assert_eq!(f.path, "a.txt");
        assert_eq!((f.added, f.removed), (Some(1), Some(1)));

        let lines = &f.hunks[0].lines;
        let added: Vec<_> = lines
            .iter()
            .filter(|l| l.kind == LineKind::Add)
            .map(|l| l.text.to_string())
            .collect();
        assert_eq!(added, ["TWO"]);
        // Old and new numbering is what a side-by-side view lays its two columns out from.
        let two = lines.iter().find(|l| l.text == "TWO").unwrap();
        assert_eq!((two.old_no, two.new_no), (None, Some(2)));
        let one = lines.iter().find(|l| l.text == "one").unwrap();
        assert_eq!((one.old_no, one.new_no), (Some(1), Some(1)));
    });
}

#[test]
fn the_first_commit_shows_its_contents_rather_than_nothing() {
    // Without --root a root commit diffs against nothing and the panel would look empty.
    let repo = TestRepo::new().write("a.txt", "hello\n").commit("first");

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let files = loc
            .commit_diff(&runner, "HEAD", &[], Context::Hunks)
            .await
            .unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].change, FileChange::Added);
        assert_eq!(files[0].hunks[0].lines[0].text, "hello");
    });
}

#[test]
fn a_merge_diffs_against_its_first_parent() {
    let repo = TestRepo::new().write("base.txt", "base\n").commit("root");
    repo.git(["checkout", "-q", "-b", "side"]);
    let repo = repo.write("side.txt", "side\n").commit("on side");
    repo.git(["checkout", "-q", "-"]);
    let repo = repo.write("main.txt", "main\n").commit("on main");
    repo.git(["merge", "--no-ff", "-m", "merge side", "side"]);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
        let files = loc
            .commit_diff(&runner, "HEAD", &[], Context::Hunks)
            .await
            .unwrap();
        // What the merge brought in relative to the branch it was merged into, which is the
        // only reading a patch parser can represent.
        let paths: Vec<_> = files.iter().map(|f| f.path.to_string()).collect();
        assert_eq!(paths, ["side.txt"]);
    });
}

/// A side-by-side view shows the change where it sits in the file, so it needs all of it.
#[test]
fn whole_file_context_carries_the_lines_no_hunk_would_reach() {
    let mut before = String::new();
    for n in 1..=60 {
        writeln!(before, "line {n}").unwrap();
    }
    let after = before.replace("line 30\n", "line 30 changed\n");
    let repo = TestRepo::new()
        .write("a.txt", &before)
        .commit("first")
        .write("a.txt", &after)
        .commit("second");

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();

        let narrow = loc
            .commit_diff(&runner, "HEAD", &["a.txt"], Context::Hunks)
            .await
            .unwrap();
        let wide = loc
            .commit_diff(&runner, "HEAD", &["a.txt"], Context::WholeFile)
            .await
            .unwrap();

        let reaches = |files: &[coral_core::diff::FileDiff], text: &str| {
            files[0]
                .hunks
                .iter()
                .flat_map(|h| h.lines.iter())
                .any(|l| l.text == text)
        };

        // Three lines either side of line 30, and nothing of the rest of the file.
        assert!(reaches(&narrow, "line 27"));
        assert!(!reaches(&narrow, "line 1"));
        assert!(!reaches(&narrow, "line 60"));

        // One hunk holding the file end to end, with the change still marked as one.
        assert_eq!(wide[0].hunks.len(), 1);
        assert!(reaches(&wide, "line 1"));
        assert!(reaches(&wide, "line 60"));
        assert_eq!((wide[0].added, wide[0].removed), (Some(1), Some(1)));
        let changed = wide[0].hunks[0]
            .lines
            .iter()
            .filter(|l| l.kind != LineKind::Context)
            .count();
        assert_eq!(changed, 2, "one line out, one line in");
    });
}
