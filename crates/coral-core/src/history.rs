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

/// Whether `query` could be an abbreviated object id.
///
/// Four is git's own floor for an abbreviation, and anything shorter is a word that happens to
/// be spelled in hex — "added", "face", "beef" — which would send every search through a
/// `rev-parse` that fails.
#[must_use]
pub fn looks_like_an_oid(query: &str) -> bool {
    let q = query.trim();
    (4..=64).contains(&q.len()) && q.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The order matches come back in, so a search reads the same way twice.
///
/// Deduplicated keeping the first occurrence: a commit whose message and author both match is
/// one result, and it is the message match that put it where it is.
#[must_use]
pub fn merged(passes: &[Vec<String>]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for pass in passes {
        for oid in pass {
            if seen.insert(oid.clone()) {
                out.push(oid.clone());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{looks_like_an_oid, merged};

    #[test]
    fn tells_an_abbreviated_id_from_a_word() {
        assert!(looks_like_an_oid("1a2b3c"));
        assert!(looks_like_an_oid("deadbeef"));
        // Hex-looking but too short to be an abbreviation git would accept.
        assert!(!looks_like_an_oid("abc"));
        assert!(!looks_like_an_oid("fix the parser"));
        assert!(!looks_like_an_oid("zzzz"));
    }

    #[test]
    fn keeps_the_first_place_a_commit_matched() {
        let by_message = vec!["a".to_owned(), "b".to_owned()];
        let by_author = vec!["b".to_owned(), "c".to_owned()];
        assert_eq!(merged(&[by_message, by_author]), ["a", "b", "c"]);
    }
}
