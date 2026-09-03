//! The binary frame the UI receives instead of JSON.
//!
//! A million rows as JSON objects would cost hundreds of megabytes in the webview and seconds
//! of parsing. A frame is a small header, a section directory, and packed typed arrays the
//! client can view without copying.

use super::store::{RowStore, flags};

/// "CRLG", little-endian.
pub const MAGIC: u32 = 0x474c_5243;
pub const VERSION: u16 = 1;

/// Rows per frame. About 70 KB, which is comfortably inside one IPC message and roughly a
/// screenful of scroll at any sane row height.
pub const ROWS_PER_FRAME: u32 = 4096;

/// Section identifiers. Kept stable: the client switches on them.
pub mod section {
    pub const LANE: u32 = 1;
    pub const FLAGS: u32 = 2;
    pub const TIME: u32 = 3;
    /// CSR offsets into [`PARENT_LANE`], `row_count + 1` entries.
    pub const PARENT_START: u32 = 4;
    pub const PARENT_LANE: u32 = 5;
    /// Object ids packed end to end, `hash_len` bytes each.
    pub const OID: u32 = 6;
}

/// Frame-level flags.
pub mod frame {
    /// No further frames follow.
    pub const FINAL: u8 = 1 << 0;
    /// Every row came from a commit-time walk and may be reordered by the topological pass.
    pub const PROVISIONAL: u8 = 1 << 1;
}

/// magic(4) version(2) sections(2) start(4) rows(4) total(4) flags(1) `hash_len`(1)
/// plus two bytes of padding, which lands the section directory on an 8-byte boundary.
const HEADER_BYTES: usize = 24;
const SECTION_BYTES: usize = 16;

/// Encodes rows `start..end` of `store`.
///
/// Every payload begins on an 8-byte boundary. That is not tidiness: a `Float64Array` view
/// over a misaligned offset throws in every browser, so the times section would be unreadable.
#[must_use]
pub fn encode(store: &RowStore, start: u32, hash_len: usize) -> Vec<u8> {
    let end = (start + ROWS_PER_FRAME).min(store.len());
    let rows = end.saturating_sub(start);
    let count = rows as usize;

    let mut lane: Vec<u8> = Vec::with_capacity(count * 2);
    let mut row_flags: Vec<u8> = Vec::with_capacity(count);
    let mut time: Vec<u8> = Vec::with_capacity(count * 8);
    let mut parent_start: Vec<u8> = Vec::with_capacity((count + 1) * 4);
    let mut parent_lane: Vec<u8> = Vec::new();
    let mut oids: Vec<u8> = Vec::with_capacity(count * hash_len);

    let mut edges: u32 = 0;
    parent_start.extend_from_slice(&edges.to_le_bytes());
    let mut provisional = rows > 0;

    for row in start..end {
        lane.extend_from_slice(&store.lane(row).unwrap_or_default().to_le_bytes());
        let f = store.flags(row);
        row_flags.push(f);
        provisional &= f & flags::PROVISIONAL != 0;

        // Seconds fit exactly in an f64 mantissa, and BigInt64Array arithmetic in JS is an
        // order of magnitude slower for no benefit here.
        #[allow(clippy::cast_precision_loss)]
        let secs = store.time(row).unwrap_or_default() as f64;
        time.extend_from_slice(&secs.to_le_bytes());

        for l in store.parent_lanes(row) {
            parent_lane.extend_from_slice(&l.to_le_bytes());
            edges += 1;
        }
        parent_start.extend_from_slice(&edges.to_le_bytes());

        match store.oid(row) {
            Some(id) => oids.extend_from_slice(id.as_bytes()),
            None => oids.extend(std::iter::repeat_n(0_u8, hash_len)),
        }
    }

    let sections = [
        (section::LANE, lane, rows),
        (section::FLAGS, row_flags, rows),
        (section::TIME, time, rows),
        (section::PARENT_START, parent_start, rows + 1),
        (section::PARENT_LANE, parent_lane, edges),
        (section::OID, oids, rows),
    ];

    let mut header_flags = 0_u8;
    if end >= store.len() {
        header_flags |= frame::FINAL;
    }
    if provisional {
        header_flags |= frame::PROVISIONAL;
    }

    let directory = HEADER_BYTES + SECTION_BYTES * sections.len();
    let mut out =
        Vec::with_capacity(directory + sections.iter().map(|s| s.1.len() + 8).sum::<usize>());

    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&u16::try_from(sections.len()).unwrap_or(0).to_le_bytes());
    out.extend_from_slice(&start.to_le_bytes());
    out.extend_from_slice(&rows.to_le_bytes());
    out.extend_from_slice(&store.len().to_le_bytes());
    out.push(header_flags);
    out.push(u8::try_from(hash_len).unwrap_or(20));
    out.extend_from_slice(&[0, 0]);
    debug_assert_eq!(
        out.len(),
        HEADER_BYTES,
        "the header must match HEADER_BYTES"
    );

    // Offsets are absolute from the start of the frame, so the client needs no arithmetic.
    let mut offset = directory;
    let mut payload_offsets = Vec::with_capacity(sections.len());
    for (_, bytes, _) in &sections {
        offset = align8(offset);
        payload_offsets.push(offset);
        offset += bytes.len();
    }

    for ((kind, bytes, elems), off) in sections.iter().zip(&payload_offsets) {
        out.extend_from_slice(&kind.to_le_bytes());
        out.extend_from_slice(&u32::try_from(*off).unwrap_or(0).to_le_bytes());
        out.extend_from_slice(&u32::try_from(bytes.len()).unwrap_or(0).to_le_bytes());
        out.extend_from_slice(&elems.to_le_bytes());
    }

    for ((_, bytes, _), off) in sections.iter().zip(&payload_offsets) {
        out.resize(*off, 0);
        out.extend_from_slice(bytes);
    }
    out
}

const fn align8(n: usize) -> usize {
    n.div_ceil(8) * 8
}

/// A decoded frame, for tests and for the Rust side of the golden fixture.
#[derive(Debug, PartialEq)]
pub struct Frame {
    pub start_row: u32,
    pub row_count: u32,
    pub total_rows: u32,
    pub flags: u8,
    pub hash_len: usize,
    pub lanes: Vec<u16>,
    pub row_flags: Vec<u8>,
    pub times: Vec<f64>,
    pub parent_start: Vec<u32>,
    pub parent_lanes: Vec<u16>,
    pub oids: Vec<u8>,
}

/// Decodes a frame, mirroring the TypeScript reader exactly.
///
/// # Errors
/// A string naming what did not match, so a golden-fixture failure says what drifted.
pub fn decode(buf: &[u8]) -> Result<Frame, String> {
    if buf.len() < HEADER_BYTES {
        return Err("frame shorter than its header".to_owned());
    }
    let u32_at = |o: usize| u32::from_le_bytes(buf[o..o + 4].try_into().unwrap_or_default());
    let u16_at = |o: usize| u16::from_le_bytes(buf[o..o + 2].try_into().unwrap_or_default());

    if u32_at(0) != MAGIC {
        return Err("bad magic".to_owned());
    }
    if u16_at(4) != VERSION {
        return Err(format!("unsupported version {}", u16_at(4)));
    }
    let section_count = u16_at(6) as usize;
    let mut frame = Frame {
        start_row: u32_at(8),
        row_count: u32_at(12),
        total_rows: u32_at(16),
        flags: buf[20],
        hash_len: buf[21] as usize,
        lanes: Vec::new(),
        row_flags: Vec::new(),
        times: Vec::new(),
        parent_start: Vec::new(),
        parent_lanes: Vec::new(),
        oids: Vec::new(),
    };

    for i in 0..section_count {
        let base = HEADER_BYTES + i * SECTION_BYTES;
        if base + SECTION_BYTES > buf.len() {
            return Err("section directory runs past the frame".to_owned());
        }
        let kind = u32_at(base);
        let offset = u32_at(base + 4) as usize;
        let len = u32_at(base + 8) as usize;
        if !offset.is_multiple_of(8) {
            return Err(format!("section {kind} is not 8-byte aligned"));
        }
        let Some(payload) = buf.get(offset..offset + len) else {
            return Err(format!("section {kind} runs past the frame"));
        };

        match kind {
            section::LANE => {
                frame.lanes = payload
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| u16::from_le_bytes(*c))
                    .collect();
            }
            section::FLAGS => frame.row_flags = payload.to_vec(),
            section::TIME => {
                frame.times = payload
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .map(|c| f64::from_le_bytes(*c))
                    .collect();
            }
            section::PARENT_START => {
                frame.parent_start = payload
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| u32::from_le_bytes(*c))
                    .collect();
            }
            section::PARENT_LANE => {
                frame.parent_lanes = payload
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| u16::from_le_bytes(*c))
                    .collect();
            }
            section::OID => frame.oids = payload.to_vec(),
            other => return Err(format!("unknown section {other}")),
        }
    }
    Ok(frame)
}
