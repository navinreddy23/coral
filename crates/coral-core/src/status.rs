use bstr::{BString, ByteSlice};

use crate::error::CoralError;

/// Paths are bytes internally, because git's are. They become strings only here, at the
/// serialization boundary, where a non-UTF-8 path is rendered lossily rather than dropped.
mod path_as_str {
    use bstr::{BString, ByteSlice};
    use serde::Serializer;

    pub fn serialize<S: Serializer>(p: &BString, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&p.to_str_lossy())
    }

    pub mod option {
        use super::{BString, ByteSlice, Serializer};

        // serde's serialize_with always passes the field by reference.
        #[allow(clippy::ref_option)]
        pub fn serialize<S: Serializer>(p: &Option<BString>, s: S) -> Result<S::Ok, S::Error> {
            match p {
                Some(p) => s.serialize_str(&p.to_str_lossy()),
                None => s.serialize_none(),
            }
        }
    }
}

/// How one side of the index/worktree pair changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "snake_case")]
pub enum Change {
    Unmodified,
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Untracked,
    Ignored,
}

impl Change {
    /// Maps one character of the porcelain v2 XY field.
    const fn from_code(c: u8) -> Option<Self> {
        Some(match c {
            b'.' => Self::Unmodified,
            b'M' => Self::Modified,
            b'A' => Self::Added,
            b'D' => Self::Deleted,
            b'R' => Self::Renamed,
            b'C' => Self::Copied,
            b'T' => Self::TypeChanged,
            _ => return None,
        })
    }
}

/// Which side of a conflict did what. Taken from the XY of a `u` entry, whose codes mean
/// something different from an ordinary entry's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "snake_case")]
pub enum ConflictKind {
    BothModified,
    BothAdded,
    BothDeleted,
    AddedByUs,
    AddedByThem,
    DeletedByUs,
    DeletedByThem,
}

impl ConflictKind {
    const fn from_xy(x: u8, y: u8) -> Option<Self> {
        Some(match (x, y) {
            (b'U', b'U') => Self::BothModified,
            (b'A', b'A') => Self::BothAdded,
            (b'D', b'D') => Self::BothDeleted,
            (b'A', b'U') => Self::AddedByUs,
            (b'U', b'A') => Self::AddedByThem,
            (b'D', b'U') => Self::DeletedByUs,
            (b'U', b'D') => Self::DeletedByThem,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct StatusEntry {
    #[serde(serialize_with = "path_as_str::serialize")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub path: BString,
    /// Where a rename or copy came from.
    #[serde(serialize_with = "path_as_str::option::serialize")]
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub orig_path: Option<BString>,
    pub index: Change,
    pub worktree: Change,
    pub conflict: Option<ConflictKind>,
    /// Rename or copy similarity, 0-100.
    pub score: Option<u8>,
    /// True when the entry is a submodule, whose state git reports separately.
    pub submodule: bool,
    /// The file's mode changed even if its content did not.
    pub mode_changed: bool,
}

impl StatusEntry {
    #[must_use]
    pub const fn is_staged(&self) -> bool {
        !matches!(
            self.index,
            Change::Unmodified | Change::Untracked | Change::Ignored
        )
    }

    #[must_use]
    pub const fn is_conflicted(&self) -> bool {
        self.conflict.is_some()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub branch: Option<String>,
    pub oid: Option<String>,
    pub upstream: Option<String>,
    pub ahead: i64,
    pub behind: i64,
    pub stash_count: u32,
    pub entries: Vec<StatusEntry>,
}

impl Status {
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn conflicted(&self) -> impl Iterator<Item = &StatusEntry> {
        self.entries.iter().filter(|e| e.is_conflicted())
    }

    /// Parses `git status --porcelain=v2 -z --branch --show-stash`.
    ///
    /// The `-z` form is not simply the newline form with different separators: a rename entry
    /// puts its original path in the *next* NUL-delimited field rather than after a `->`, so
    /// records cannot be handled independently of one another.
    ///
    /// # Errors
    /// [`CoralError::Protocol`] on a record that does not match the documented grammar.
    pub fn parse(input: &[u8]) -> Result<Self, CoralError> {
        let mut status = Self::default();
        let mut records = input.split(|b| *b == 0).filter(|r| !r.is_empty());

        while let Some(record) = records.next() {
            match record.first() {
                Some(b'#') => status.parse_header(record),
                Some(b'1') => status.entries.push(parse_ordinary(record)?),
                Some(b'2') => {
                    // The original path is the following field, not part of this record.
                    let orig = records.next().ok_or_else(|| {
                        protocol("rename entry was not followed by its original path")
                    })?;
                    status.entries.push(parse_rename(record, orig)?);
                }
                Some(b'u') => status.entries.push(parse_unmerged(record)?),
                Some(b'?') => status.entries.push(parse_flag(record, Change::Untracked)),
                Some(b'!') => status.entries.push(parse_flag(record, Change::Ignored)),
                _ => return Err(protocol("unrecognised status record")),
            }
        }
        Ok(status)
    }

    fn parse_header(&mut self, record: &[u8]) {
        let text = record.to_str_lossy();
        let mut parts = text.split(' ');
        match (parts.next(), parts.next()) {
            (Some("#"), Some("branch.oid")) => self.oid = parts.next().map(str::to_owned),
            (Some("#"), Some("branch.head")) => {
                // "(detached)" is git's placeholder, not a branch name.
                self.branch = parts
                    .next()
                    .filter(|h| *h != "(detached)")
                    .map(str::to_owned);
            }
            (Some("#"), Some("branch.upstream")) => self.upstream = parts.next().map(str::to_owned),
            (Some("#"), Some("branch.ab")) => {
                self.ahead = parts.next().and_then(parse_signed).unwrap_or(0);
                self.behind = parts.next().and_then(parse_signed).unwrap_or(0);
            }
            (Some("#"), Some("stash")) => {
                self.stash_count = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0);
            }
            _ => {}
        }
    }
}

/// `+1` / `-3` — the sign is part of the token and the count is always non-negative.
fn parse_signed(token: &str) -> Option<i64> {
    let (sign, digits) = token.split_at(1);
    let n: i64 = digits.parse().ok()?;
    Some(if sign == "-" { -n } else { n })
}

/// `1 XY sub mH mI mW hH hI path`
fn parse_ordinary(record: &[u8]) -> Result<StatusEntry, CoralError> {
    let f = fields(record, 8)?;
    let (index, worktree) = xy(f[1])?;
    Ok(StatusEntry {
        path: BString::from(f[8]),
        orig_path: None,
        index,
        worktree,
        conflict: None,
        score: None,
        submodule: is_submodule(f[2]),
        mode_changed: f[3] != f[4] || f[4] != f[5],
    })
}

/// `2 XY sub mH mI mW hH hI X<score> path` with the original path in the next field.
fn parse_rename(record: &[u8], orig: &[u8]) -> Result<StatusEntry, CoralError> {
    let f = fields(record, 9)?;
    let (index, worktree) = xy(f[1])?;
    Ok(StatusEntry {
        path: BString::from(f[9]),
        orig_path: Some(BString::from(orig)),
        index,
        worktree,
        conflict: None,
        // "R100" / "C75": the leading letter repeats the change kind.
        score: f[8]
            .get(1..)
            .and_then(|d| d.to_str().ok())
            .and_then(|d| d.parse().ok()),
        submodule: is_submodule(f[2]),
        mode_changed: f[3] != f[4] || f[4] != f[5],
    })
}

/// `u XY sub m1 m2 m3 mW h1 h2 h3 path`
fn parse_unmerged(record: &[u8]) -> Result<StatusEntry, CoralError> {
    let f = fields(record, 10)?;
    let xy = f[1];
    let (x, y) = (
        *xy.first()
            .ok_or_else(|| protocol("unmerged entry has no XY"))?,
        *xy.get(1)
            .ok_or_else(|| protocol("unmerged entry has a one-character XY"))?,
    );
    Ok(StatusEntry {
        path: BString::from(f[10]),
        orig_path: None,
        index: Change::Modified,
        worktree: Change::Modified,
        conflict: Some(ConflictKind::from_xy(x, y).ok_or_else(|| protocol("unknown conflict XY"))?),
        score: None,
        submodule: is_submodule(f[2]),
        mode_changed: false,
    })
}

/// `? path` and `! path`, whose payload is the whole rest of the record.
fn parse_flag(record: &[u8], change: Change) -> StatusEntry {
    StatusEntry {
        path: BString::from(record.get(2..).unwrap_or_default()),
        orig_path: None,
        index: change,
        worktree: change,
        conflict: None,
        score: None,
        submodule: false,
        mode_changed: false,
    }
}

/// Splits into `count` space-delimited fields plus the path, which may itself contain spaces
/// and so is never split.
fn fields(record: &[u8], count: usize) -> Result<Vec<&[u8]>, CoralError> {
    let mut out = Vec::with_capacity(count + 1);
    let mut rest = record;
    for _ in 0..count {
        let i = rest
            .find_byte(b' ')
            .ok_or_else(|| protocol("status record has too few fields"))?;
        out.push(&rest[..i]);
        rest = &rest[i + 1..];
    }
    out.push(rest);
    Ok(out)
}

fn xy(field: &[u8]) -> Result<(Change, Change), CoralError> {
    let x = field.first().copied().and_then(Change::from_code);
    let y = field.get(1).copied().and_then(Change::from_code);
    match (x, y) {
        (Some(x), Some(y)) => Ok((x, y)),
        _ => Err(protocol("unknown XY code")),
    }
}

/// The submodule field is `N...` for a normal file and `S<c><m><u>` for a submodule.
fn is_submodule(field: &[u8]) -> bool {
    field.first() == Some(&b'S')
}

fn protocol(detail: &str) -> CoralError {
    CoralError::Protocol {
        label: "status",
        detail: detail.to_owned(),
    }
}
