use bstr::{BString, ByteSlice};

use crate::commit::{Commit, Signature};
use crate::error::CoralError;

/// Field order for [`parse`]. NUL between every field *and* between records, so a message
/// containing newlines cannot be mistaken for a record boundary.
pub const FORMAT: &str = "%H%x00%P%x00%an%x00%ae%x00%at%x00%cn%x00%ce%x00%ct%x00%s%x00%b";

const FIELDS: usize = 10;

/// Parses `git log -z --format=<FORMAT>`.
///
/// # Errors
/// [`CoralError::Protocol`] if the field count is not a multiple of the format's.
pub fn parse(input: &[u8]) -> Result<Vec<Commit>, CoralError> {
    // A trailing NUL after the final record leaves one empty tail field.
    let fields: Vec<&[u8]> = input.split(|b| *b == 0).collect();
    let fields = match fields.split_last() {
        Some((&[], head)) => head,
        _ => &fields[..],
    };
    if fields.is_empty() {
        return Ok(Vec::new());
    }
    if fields.len() % FIELDS != 0 {
        return Err(CoralError::Protocol {
            label: "log",
            detail: format!(
                "expected a multiple of {FIELDS} fields, got {}",
                fields.len()
            ),
        });
    }

    fields
        .as_chunks::<FIELDS>()
        .0
        .iter()
        .map(|c| parse_one(c))
        .collect()
}

fn parse_one(f: &[&[u8]]) -> Result<Commit, CoralError> {
    let text = |i: usize| String::from_utf8_lossy(f[i]).into_owned();
    let time = |i: usize| -> Result<i64, CoralError> {
        f[i].to_str()
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .ok_or_else(|| CoralError::Protocol {
                label: "log",
                detail: "malformed timestamp".to_owned(),
            })
    };

    Ok(Commit {
        oid: text(0).trim().to_owned(),
        parents: f[1]
            .to_str_lossy()
            .split_whitespace()
            .map(str::to_owned)
            .collect(),
        author: Signature {
            name: text(2),
            email: text(3),
            time: time(4)?,
        },
        committer: Signature {
            name: text(5),
            email: text(6),
            time: time(7)?,
        },
        summary: BString::from(f[8]),
        // git separates records with NUL but still puts a newline after the body.
        body: BString::from(f[9].strip_suffix(b"\n").unwrap_or(f[9])),
    })
}

/// Narrows a history query.
#[derive(Clone, Debug, Default)]
pub struct LogQuery {
    pub rev: Option<String>,
    pub path: Option<String>,
    pub author: Option<String>,
    /// Matched against the message, as `--grep`.
    pub grep: Option<String>,
    /// Matched against added or removed content, as `-S`.
    pub pickaxe: Option<String>,
    pub limit: Option<u64>,
    /// Follow a file across renames. Only valid with a single path.
    pub follow: bool,
}
