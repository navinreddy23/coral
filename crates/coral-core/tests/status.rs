//! Table-driven tests over verbatim `--porcelain=v2 -z` output captured from git 2.43, plus
//! live fixtures so the captures cannot silently drift from what git actually emits.

use coral_core::status::{Change, ConflictKind, Status};
use coral_core::testutil::TestRepo;

/// Builds a `-z` stream from records, since NUL is awkward to write inline.
fn z(records: &[&str]) -> Vec<u8> {
    let mut out = Vec::new();
    for r in records {
        out.extend_from_slice(r.as_bytes());
        out.push(0);
    }
    out
}

#[test]
fn parses_branch_headers_including_upstream_and_stash() {
    let input = z(&[
        "# branch.oid 6a87e8b3182e6fc140851b5d0db20bcd6a39522b",
        "# branch.head main",
        "# branch.upstream origin/main",
        "# branch.ab +3 -2",
        "# stash 1",
    ]);
    let s = Status::parse(&input).unwrap();

    assert_eq!(s.branch.as_deref(), Some("main"));
    assert_eq!(s.upstream.as_deref(), Some("origin/main"));
    assert_eq!((s.ahead, s.behind), (3, -2));
    assert_eq!(s.stash_count, 1);
    assert!(s.is_clean());
}

/// git writes the literal "(detached)" in branch.head, which is not a branch name.
#[test]
fn detached_head_is_not_reported_as_a_branch() {
    let input = z(&["# branch.oid abc", "# branch.head (detached)"]);
    assert_eq!(Status::parse(&input).unwrap().branch, None);
}

#[test]
fn parses_every_ordinary_entry_shape() {
    let input = z(&[
        "1 A. N... 000000 100644 100644 0000000000000000000000000000000000000000 3e757656cf36eca53338e520d134963a44f793f8 added.txt",
        "1 D. N... 100644 000000 000000 abaddc0b9edd523c69166a2c9f3a9e31a4c873e3 0000000000000000000000000000000000000000 deleted.txt",
        "1 .M N... 100644 100644 100755 2680cfddbd9fa03c059ac60d2bec5e59a1c34281 2680cfddbd9fa03c059ac60d2bec5e59a1c34281 modified.txt",
        "1 MM N... 100644 100644 100644 df967b96a579e45a18b8251732d16804b2e56a55 910c9bc19d38590942c02e28428093452fe66610 tracked.txt",
        "? untracked.txt",
        "! ignored.txt",
    ]);
    let s = Status::parse(&input).unwrap();
    assert_eq!(s.entries.len(), 6);

    assert_eq!(s.entries[0].index, Change::Added);
    assert_eq!(s.entries[0].worktree, Change::Unmodified);
    assert_eq!(s.entries[1].index, Change::Deleted);

    // A chmod with identical blob hashes: only the mode moved.
    assert_eq!(s.entries[2].worktree, Change::Modified);
    assert!(
        s.entries[2].mode_changed,
        "100644 -> 100755 is a mode change"
    );

    // Staged and then modified again.
    assert_eq!(s.entries[3].index, Change::Modified);
    assert_eq!(s.entries[3].worktree, Change::Modified);
    assert!(s.entries[3].is_staged());

    assert_eq!(s.entries[4].index, Change::Untracked);
    assert_eq!(s.entries[5].index, Change::Ignored);
}

/// Under `-z` the original path is a separate record, not a `->` suffix. Getting this wrong
/// silently shifts every following entry by one.
#[test]
fn a_rename_takes_its_original_path_from_the_following_record() {
    let input = z(&[
        "2 R. N... 100644 100644 100644 9d80ddb4cc7365318accecc8f8084993ecf72e69 9d80ddb4cc7365318accecc8f8084993ecf72e69 R100 renamed-new.txt",
        "renamed.txt",
        "? after.txt",
    ]);
    let s = Status::parse(&input).unwrap();

    assert_eq!(
        s.entries.len(),
        2,
        "the original path must not become its own entry"
    );
    assert_eq!(s.entries[0].path, "renamed-new.txt");
    assert_eq!(
        s.entries[0]
            .orig_path
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some("renamed.txt")
    );
    assert_eq!(s.entries[0].index, Change::Renamed);
    assert_eq!(s.entries[0].score, Some(100));
    assert_eq!(
        s.entries[1].path, "after.txt",
        "the entry after a rename must not shift"
    );
}

#[test]
fn parses_every_conflict_kind() {
    let input = z(&[
        "u AA N... 000000 100644 100644 100644 0000000000000000000000000000000000000000 9b4be140fe02cfe8bed759848b74f1988f75a242 b6835df1f8a46ecd6ceb504296738c5169f27f14 addadd.txt",
        "u UU N... 100644 100644 100644 100644 df967b96a579e45a18b8251732d16804b2e56a55 ba2906d0666cf726c7eaadd2cd3db615dedfdf3a 2299c37978265a95cbe835a4b0f0bbf15aad5549 both.txt",
        "u UD N... 100644 100644 000000 100644 587be6b4c3f93f93c489c0111bba5596147a26cb 5ea2ed416fbd4a4cbe227b75fe255dd7fa6bd4d6 0000000000000000000000000000000000000000 delmod.txt",
    ]);
    let s = Status::parse(&input).unwrap();

    let kinds: Vec<_> = s.conflicted().map(|e| e.conflict.unwrap()).collect();
    assert_eq!(
        kinds,
        vec![
            ConflictKind::BothAdded,
            ConflictKind::BothModified,
            ConflictKind::DeletedByThem
        ]
    );
    assert_eq!(s.conflicted().count(), 3);
}

#[test]
fn paths_with_spaces_survive_because_the_path_is_never_split() {
    let input = z(&[
        "1 M. N... 100644 100644 100644 aaaa bbbb a file with spaces.txt",
        "? another one.txt",
    ]);
    let s = Status::parse(&input).unwrap();
    assert_eq!(s.entries[0].path, "a file with spaces.txt");
    assert_eq!(s.entries[1].path, "another one.txt");
}

#[test]
fn a_submodule_entry_is_flagged() {
    let input = z(&["1 .M SC.. 160000 160000 160000 aaaa bbbb vendor/lib"]);
    let s = Status::parse(&input).unwrap();
    assert!(s.entries[0].submodule);
}

#[test]
fn malformed_records_are_rejected_rather_than_guessed_at() {
    assert!(Status::parse(&z(&["1 M."])).is_err(), "too few fields");
    assert!(
        Status::parse(&z(&["x nonsense"])).is_err(),
        "unknown record type"
    );
    assert!(
        Status::parse(&z(&["1 ZZ N... 1 2 3 a b p"])).is_err(),
        "unknown XY"
    );
    assert!(
        Status::parse(&z(&["2 R. N... 1 2 3 a b R100 new.txt"])).is_err(),
        "rename with no following original path"
    );
}

#[test]
fn an_empty_stream_is_a_clean_repository() {
    let s = Status::parse(b"").unwrap();
    assert!(s.is_clean());
    assert_eq!(s.ahead, 0);
}

/// The captures above are only trustworthy while git still emits that shape, so this runs the
/// real command and asserts the parser handles whatever comes back.
#[test]
fn parses_live_output_from_a_real_repository() {
    let r = TestRepo::new()
        .write("keep.txt", "a\n")
        .write("gone.txt", "b\n")
        .commit("base");
    r.git(["rm", "-q", "gone.txt"]);
    let r = r.write("keep.txt", "changed\n").write("new.txt", "n\n");
    r.git(["add", "new.txt"]);
    std::fs::write(r.path().join("untracked.txt"), "u\n").unwrap();

    let raw = r.git_bytes([
        "--no-optional-locks",
        "status",
        "--porcelain=v2",
        "-z",
        "--branch",
        "--show-stash",
    ]);
    let s = Status::parse(&raw).unwrap();

    assert_eq!(s.branch.as_deref(), Some("main"));
    let paths: Vec<String> = s.entries.iter().map(|e| e.path.to_string()).collect();
    assert!(paths.contains(&"gone.txt".to_owned()));
    assert!(paths.contains(&"keep.txt".to_owned()));
    assert!(paths.contains(&"new.txt".to_owned()));
    assert!(paths.contains(&"untracked.txt".to_owned()));
}
