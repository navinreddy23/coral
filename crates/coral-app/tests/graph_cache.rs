//! The two-phase graph sequence the app performs on open, and the framing that carries it.

use coral_core::graph::{GixCommitStream, StreamOpts, build, wire};
use coral_core::testutil::TestRepo;

/// Mirrors what `GraphCache` builds, so the sequence is exercised without Tauri.
fn build_store(path: &std::path::Path, first_paint: bool) -> coral_core::graph::RowStore {
    let stream = GixCommitStream::open(path).unwrap();
    let opts = if first_paint {
        StreamOpts::first_paint(u64::from(wire::ROWS_PER_FRAME))
    } else {
        StreamOpts::default()
    };
    build(&stream, &opts).unwrap()
}

/// Paint fast, then replace with topologically correct rows. Serving the first store back to
/// the second request would leave the user on commit-time order for good.
#[test]
fn the_second_phase_produces_rows_that_are_no_longer_provisional() {
    let repo = TestRepo::new().write("a.txt", "base\n").commit("base");
    repo.git(["checkout", "--quiet", "-b", "side"]);
    let repo = repo.write("s.txt", "s\n").commit("side");
    repo.git(["checkout", "--quiet", "main"]);
    let repo = repo.write("m.txt", "m\n").commit("main");
    repo.git(["merge", "--quiet", "--no-ff", "-m", "merge", "side"]);

    let quick = build_store(repo.path(), true);
    let full = build_store(repo.path(), false);
    let provisional = coral_core::graph::flags::PROVISIONAL;

    assert!((0..quick.len()).all(|r| quick.flags(r) & provisional != 0));
    assert!((0..full.len()).all(|r| full.flags(r) & provisional == 0));
    assert_eq!(quick.len(), full.len(), "both phases see the same commits");

    let quick_frame = wire::decode(&wire::encode(&quick, 0, 20)).unwrap();
    let full_frame = wire::decode(&wire::encode(&full, 0, 20)).unwrap();
    assert!(quick_frame.flags & wire::frame::PROVISIONAL != 0);
    assert_eq!(full_frame.flags & wire::frame::PROVISIONAL, 0);
}

#[test]
fn frames_tile_the_whole_graph_without_gaps_or_overlap() {
    let mut repo = TestRepo::new().write("a.txt", "0\n").commit("commit 0");
    for i in 1..40 {
        repo = repo
            .write("a.txt", &format!("{i}\n"))
            .commit(&format!("commit {i}"));
    }
    let store = build_store(repo.path(), false);

    let mut seen = 0_u32;
    let mut start = 0_u32;
    loop {
        let frame = wire::decode(&wire::encode(&store, start, 20)).unwrap();
        assert_eq!(frame.start_row, start);
        seen += frame.row_count;
        if frame.flags & wire::frame::FINAL != 0 {
            break;
        }
        assert!(frame.row_count > 0, "a non-final frame must make progress");
        start += frame.row_count;
    }
    assert_eq!(seen, store.len(), "every row appeared in exactly one frame");
}
