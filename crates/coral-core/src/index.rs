use bstr::{BString, ByteSlice};

use crate::diff::{FileDiff, Hunk, LineKind};
use crate::error::CoralError;

/// Which lines of a hunk to apply. Line numbers are indices into [`Hunk::lines`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    /// Every changed line in the hunk.
    WholeHunk,
    /// Only these lines; every other changed line is treated as unchanged.
    Lines(Vec<usize>),
}

impl Selection {
    fn includes(&self, i: usize) -> bool {
        match self {
            Self::WholeHunk => true,
            Self::Lines(v) => v.contains(&i),
        }
    }
}

/// Which direction a generated patch runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Worktree into the index.
    Stage,
    /// Index back out, which git applies in reverse.
    Unstage,
}

/// Builds a patch containing only the selected hunks of one file, suitable for
/// `git apply --cached`.
///
/// The hard part is not filtering lines but recomputing the header. A `+` line that is not
/// selected disappears entirely, while a `-` line that is not selected becomes context — it
/// still exists on both sides. Getting that backwards produces a patch that applies cleanly
/// and silently stages the wrong thing.
///
/// # Errors
/// [`CoralError::Protocol`] if a requested hunk does not exist.
pub fn build_patch(
    file: &FileDiff,
    hunks: &[(usize, Selection)],
    direction: Direction,
) -> Result<BString, CoralError> {
    let mut out = BString::from(Vec::new());
    let old_path = file.old_path.as_ref().unwrap_or(&file.path);

    out.extend_from_slice(b"diff --git a/");
    out.extend_from_slice(old_path);
    out.extend_from_slice(b" b/");
    out.extend_from_slice(&file.path);
    out.push(b'\n');
    out.extend_from_slice(b"--- a/");
    out.extend_from_slice(old_path);
    out.push(b'\n');
    out.extend_from_slice(b"+++ b/");
    out.extend_from_slice(&file.path);
    out.push(b'\n');

    // Each retained hunk shifts the lines after it on the side being rebuilt.
    let mut drift: i64 = 0;
    for (index, selection) in hunks {
        let hunk = file.hunks.get(*index).ok_or_else(|| CoralError::Protocol {
            label: "apply",
            detail: format!("hunk {index} does not exist"),
        })?;
        let rendered = render_hunk(hunk, selection, direction, &mut drift);
        out.extend_from_slice(&rendered);
    }
    Ok(out)
}

/// Emits one hunk with only `selection` applied.
fn render_hunk(
    hunk: &Hunk,
    selection: &Selection,
    direction: Direction,
    drift: &mut i64,
) -> BString {
    let mut body: Vec<u8> = Vec::new();
    let mut old_count: u32 = 0;
    let mut new_count: u32 = 0;

    for (i, line) in hunk.lines.iter().enumerate() {
        let selected = selection.includes(i);
        let (marker, counts_old, counts_new) = match (line.kind, selected) {
            (LineKind::Add, true) => (b'+', false, true),
            (LineKind::Remove, true) => (b'-', true, false),
            // An unselected addition never happened, so it is absent from both sides.
            (LineKind::Add, false) => continue,
            // Context, and an unselected removal, are both present on both sides: declining to
            // stage a removal means the line is still there afterwards.
            (LineKind::Context, _) | (LineKind::Remove, false) => (b' ', true, true),
        };

        body.push(marker);
        body.extend_from_slice(&line.text);
        body.push(b'\n');
        if line.no_newline {
            body.extend_from_slice(b"\\ No newline at end of file\n");
        }
        old_count += u32::from(counts_old);
        new_count += u32::from(counts_new);
    }

    // Nothing changed once the selection was applied; emitting it would be a no-op hunk that
    // `git apply` rejects as empty.
    if old_count == new_count && !body.iter().any(|b| *b == b'+' || *b == b'-') {
        return BString::from(Vec::new());
    }

    // Applying in reverse means git reads the header the other way round, so the start we must
    // shift is the one that side is rebuilding.
    let (old_start, new_start) = match direction {
        Direction::Stage => (
            hunk.old_start,
            u32::try_from(i64::from(hunk.old_start) + *drift).unwrap_or(hunk.new_start),
        ),
        Direction::Unstage => (hunk.old_start, hunk.new_start),
    };
    *drift += i64::from(new_count) - i64::from(old_count);

    let header = format!("@@ -{old_start},{old_count} +{new_start},{new_count} @@\n");
    let mut out = BString::from(header.into_bytes());
    out.extend_from_slice(&body);
    out
}

/// The arguments `git apply` needs for a generated patch.
///
/// `--unidiff-zero` is required because a selection can leave a hunk with no context lines at
/// all, which git otherwise refuses.
#[must_use]
pub fn apply_args(direction: Direction) -> Vec<&'static str> {
    let mut v = vec!["apply", "--cached", "--unidiff-zero", "--whitespace=nowarn"];
    if direction == Direction::Unstage {
        v.push("--reverse");
    }
    v
}

/// True when the patch would do nothing, so the caller can skip spawning git.
#[must_use]
pub fn is_empty_patch(patch: &BString) -> bool {
    !patch.lines().any(|l| l.starts_with(b"@@"))
}
