//! The binary frame, and the golden fixture the TypeScript decoder is tested against.
//!
//! Encoder and decoder living in one language prove only self-consistency. The golden file
//! committed here is read by `ui/tests/frame.test.ts` as well, so a layout change that only
//! one side hears about fails on the other.

use coral_core::graph::wire::{Frame, MAGIC, VERSION, decode, encode, frame, section};
use coral_core::graph::{GixCommitStream, StreamOpts, build};
use coral_core::testutil::TestRepo;

/// Where the section directory starts, i.e. the header size.
const DIRECTORY_AT: usize = 24;

/// Where the shared fixture lives, relative to the crate.
const GOLDEN: &str = "../../ui/tests/fixtures/frame.bin";

/// A small DAG with a merge, a fork and two roots, so the frame carries real edges.
fn fixture_store() -> coral_core::graph::RowStore {
    let r = TestRepo::new().write("a.txt", "base\n").commit("base");
    r.git(["checkout", "--quiet", "-b", "side"]);
    let r = r.write("s.txt", "s\n").commit("side");
    r.git(["checkout", "--quiet", "main"]);
    let r = r.write("m.txt", "m\n").commit("main");
    r.git(["merge", "--quiet", "--no-ff", "-m", "merge", "side"]);
    r.git(["checkout", "--quiet", "--orphan", "orphan"]);
    r.git(["rm", "-rf", "--quiet", "--cached", "."]);
    let r = r.write("o.txt", "o\n").commit("orphan root");
    r.git(["checkout", "--quiet", "main"]);

    let stream = GixCommitStream::open(r.path()).unwrap();
    build(&stream, &StreamOpts::default()).unwrap()
}

fn encoded() -> Vec<u8> {
    encode(&fixture_store(), 0, 20)
}

#[test]
fn a_frame_round_trips_through_its_own_decoder() {
    let store = fixture_store();
    let bytes = encode(&store, 0, 20);
    let f = decode(&bytes).unwrap();

    assert_eq!(f.start_row, 0);
    assert_eq!(f.row_count, store.len());
    assert_eq!(f.total_rows, store.len());
    assert_eq!(f.hash_len, 20);
    assert_eq!(f.lanes.len(), store.len() as usize);
    assert_eq!(f.row_flags.len(), store.len() as usize);
    assert_eq!(f.times.len(), store.len() as usize);
    assert_eq!(f.parent_start.len(), store.len() as usize + 1);
    assert_eq!(f.oids.len(), store.len() as usize * 20);

    for row in 0..store.len() {
        assert_eq!(f.lanes[row as usize], store.lane(row).unwrap());
        assert_eq!(f.row_flags[row as usize], store.flags(row));
        let start = f.parent_start[row as usize] as usize;
        let end = f.parent_start[row as usize + 1] as usize;
        assert_eq!(&f.parent_lanes[start..end], store.parent_lanes(row));
        let oid_at = row as usize * 20;
        assert_eq!(
            &f.oids[oid_at..oid_at + 20],
            store.oid(row).unwrap().as_bytes()
        );
    }
}

/// A `Float64Array` view over a misaligned offset throws in every browser, so the times
/// section would be unreadable rather than merely slow.
#[test]
fn every_section_payload_is_eight_byte_aligned() {
    let bytes = encoded();
    let count = u16::from_le_bytes([bytes[6], bytes[7]]) as usize;

    for i in 0..count {
        let base = DIRECTORY_AT + i * 16;
        let kind = u32::from_le_bytes(bytes[base..base + 4].try_into().unwrap());
        let offset = u32::from_le_bytes(bytes[base + 4..base + 8].try_into().unwrap());
        assert_eq!(offset % 8, 0, "section {kind} sits at offset {offset}");
    }
}

#[test]
fn the_header_identifies_itself() {
    let bytes = encoded();
    assert_eq!(u32::from_le_bytes(bytes[0..4].try_into().unwrap()), MAGIC);
    assert_eq!(u16::from_le_bytes(bytes[4..6].try_into().unwrap()), VERSION);
    assert_eq!(
        u16::from_le_bytes(bytes[6..8].try_into().unwrap()),
        7,
        "seven sections"
    );
}

#[test]
fn a_complete_walk_is_marked_final_and_not_provisional() {
    let f = decode(&encoded()).unwrap();
    assert!(
        f.flags & frame::FINAL != 0,
        "one frame covers this whole graph"
    );
    assert!(
        f.flags & frame::PROVISIONAL == 0,
        "a topological build is not provisional"
    );
}

#[test]
fn a_first_paint_frame_is_marked_provisional() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("one");
    let stream = GixCommitStream::open(repo.path()).unwrap();
    let store = build(&stream, &StreamOpts::first_paint(4096)).unwrap();

    let f = decode(&encode(&store, 0, 20)).unwrap();
    assert!(f.flags & frame::PROVISIONAL != 0);
}

#[test]
fn a_frame_past_the_end_is_empty_but_still_valid() {
    let store = fixture_store();
    let f = decode(&encode(&store, store.len() + 100, 20)).unwrap();

    assert_eq!(f.row_count, 0);
    assert!(f.lanes.is_empty());
    assert_eq!(
        f.parent_start,
        vec![0],
        "the CSR array always has its leading zero"
    );
}

#[test]
fn a_truncated_or_corrupt_frame_is_rejected_rather_than_misread() {
    let bytes = encoded();
    assert!(decode(&bytes[..10]).is_err(), "shorter than the header");

    let mut wrong_magic = bytes.clone();
    wrong_magic[0] ^= 0xff;
    assert!(decode(&wrong_magic).is_err());

    let mut wrong_version = bytes.clone();
    wrong_version[4] = 99;
    assert!(decode(&wrong_version).is_err());

    let mut bad_section = bytes;
    bad_section[DIRECTORY_AT] = 200; // an unknown section kind
    assert!(decode(&bad_section).is_err());
}

/// Writes the fixture the TypeScript decoder reads. Run with `CORAL_UPDATE_GOLDEN=1` after a
/// deliberate layout change; otherwise this asserts the bytes have not moved.
#[test]
fn the_golden_frame_matches_the_committed_fixture() {
    let bytes = encoded();
    let path = std::path::Path::new(GOLDEN);

    if std::env::var("CORAL_UPDATE_GOLDEN").is_ok() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &bytes).unwrap();
    }

    let committed = std::fs::read(path).unwrap_or_else(|_| {
        panic!("{GOLDEN} is missing; run with CORAL_UPDATE_GOLDEN=1 to write it")
    });
    assert_eq!(
        bytes, committed,
        "the frame layout changed; ui/src/graph/frame.ts must change with it, then rerun with \
         CORAL_UPDATE_GOLDEN=1"
    );

    // Whatever the TypeScript side reads, this is what it should find.
    let f: Frame = decode(&committed).unwrap();
    assert_eq!(f.row_count, 5, "base, side, main, merge, orphan root");
    assert_eq!(f.total_rows, 5);
    assert_eq!(f.hash_len, 20);
    assert!(
        f.row_flags
            .iter()
            .any(|b| b & coral_core::graph::flags::MERGE != 0)
    );
    assert_eq!(
        f.row_flags
            .iter()
            .filter(|b| *b & coral_core::graph::flags::ROOT != 0)
            .count(),
        2
    );
    assert_eq!(section::LANE, 1, "section ids are part of the contract");
}
