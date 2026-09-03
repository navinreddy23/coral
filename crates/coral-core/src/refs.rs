use crate::error::CoralError;

/// What kind of thing a ref names.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RefKind {
    LocalBranch,
    RemoteBranch { remote: String },
    Tag { annotated: bool },
    Stash,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct GitRef {
    /// Full name, e.g. `refs/heads/main`.
    pub name: String,
    /// The name the UI shows, e.g. `main` or `origin/main`.
    pub short: String,
    pub kind: RefKind,
    /// What the ref points at. For an annotated tag this is the tag object.
    pub target: String,
    /// The commit an annotated tag ultimately points at. `None` for everything else.
    pub peeled: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
}

impl GitRef {
    /// The commit this ref should be drawn on.
    #[must_use]
    pub fn commit(&self) -> &str {
        self.peeled.as_deref().unwrap_or(&self.target)
    }
}

/// The format string [`parse`] expects. Fields are NUL-separated; records are newline
/// separated, which is safe because git forbids newlines in ref names.
///
/// `for-each-ref` has no `-z` before git 2.48, so NUL has to be embedded in the format itself
/// rather than requested as a flag.
pub const FORMAT: &str =
    "%(refname)%00%(objectname)%00%(objecttype)%00%(upstream)%00%(upstream:track)%00%(*objectname)";

/// Parses `git for-each-ref --format=<FORMAT>`.
///
/// # Errors
/// [`CoralError::Protocol`] if a record does not have the expected field count.
pub fn parse(input: &[u8]) -> Result<Vec<GitRef>, CoralError> {
    input
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .map(parse_line)
        .collect()
}

fn parse_line(line: &[u8]) -> Result<GitRef, CoralError> {
    let text = String::from_utf8_lossy(line);
    let f: Vec<&str> = text.split('\0').collect();
    if f.len() < 6 {
        return Err(CoralError::Protocol {
            label: "for-each-ref",
            detail: format!("expected 6 fields, got {}", f.len()),
        });
    }

    let name = f[0].to_owned();
    let annotated = f[2] == "tag";
    let (kind, short) = classify(&name, annotated);
    let (ahead, behind) = parse_track(f[4]);

    Ok(GitRef {
        short,
        kind,
        target: f[1].to_owned(),
        peeled: (!f[5].is_empty()).then(|| f[5].to_owned()),
        upstream: (!f[3].is_empty()).then(|| strip_remote_prefix(f[3]).to_owned()),
        ahead,
        behind,
        name,
    })
}

fn classify(name: &str, annotated: bool) -> (RefKind, String) {
    if let Some(short) = name.strip_prefix("refs/heads/") {
        return (RefKind::LocalBranch, short.to_owned());
    }
    if let Some(short) = name.strip_prefix("refs/remotes/") {
        // The first segment is the remote; the rest is the branch.
        let remote = short.split('/').next().unwrap_or_default().to_owned();
        return (RefKind::RemoteBranch { remote }, short.to_owned());
    }
    if let Some(short) = name.strip_prefix("refs/tags/") {
        return (RefKind::Tag { annotated }, short.to_owned());
    }
    if name == "refs/stash" {
        return (RefKind::Stash, "stash".to_owned());
    }
    (RefKind::Other, name.to_owned())
}

fn strip_remote_prefix(name: &str) -> &str {
    name.strip_prefix("refs/remotes/").unwrap_or(name)
}

/// `[ahead 3, behind 2]`, `[ahead 1]`, `[gone]`, or empty.
fn parse_track(field: &str) -> (u32, u32) {
    let inner = field.trim_start_matches('[').trim_end_matches(']');
    let mut ahead = 0;
    let mut behind = 0;
    for part in inner.split(',') {
        let part = part.trim();
        if let Some(n) = part.strip_prefix("ahead ") {
            ahead = n.parse().unwrap_or(0);
        } else if let Some(n) = part.strip_prefix("behind ") {
            behind = n.parse().unwrap_or(0);
        }
    }
    (ahead, behind)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(fields: &[&str]) -> Vec<u8> {
        fields.join("\0").into_bytes()
    }

    #[test]
    fn classifies_branches_remotes_tags_and_stash() {
        let input = [
            line(&[
                "refs/heads/main",
                "aaa",
                "commit",
                "refs/remotes/origin/main",
                "",
                "",
            ]),
            line(&["refs/remotes/origin/feature/x", "bbb", "commit", "", "", ""]),
            line(&["refs/tags/v1.0", "ccc", "tag", "", "", "ddd"]),
            line(&["refs/tags/light", "eee", "commit", "", "", ""]),
            line(&["refs/stash", "fff", "commit", "", "", ""]),
        ]
        .join(&b'\n');

        let refs = parse(&input).unwrap();
        assert_eq!(refs[0].kind, RefKind::LocalBranch);
        assert_eq!(refs[0].short, "main");
        assert_eq!(refs[0].upstream.as_deref(), Some("origin/main"));

        assert_eq!(
            refs[1].kind,
            RefKind::RemoteBranch {
                remote: "origin".to_owned()
            }
        );
        assert_eq!(
            refs[1].short, "origin/feature/x",
            "a branch name may contain slashes"
        );

        assert_eq!(refs[2].kind, RefKind::Tag { annotated: true });
        assert_eq!(
            refs[2].commit(),
            "ddd",
            "an annotated tag is drawn on its peeled commit"
        );
        assert_eq!(refs[3].kind, RefKind::Tag { annotated: false });
        assert_eq!(refs[3].commit(), "eee");

        assert_eq!(refs[4].kind, RefKind::Stash);
    }

    #[test]
    fn parses_every_upstream_track_shape() {
        assert_eq!(parse_track(""), (0, 0));
        assert_eq!(parse_track("[ahead 1]"), (1, 0));
        assert_eq!(parse_track("[behind 4]"), (0, 4));
        assert_eq!(parse_track("[ahead 3, behind 2]"), (3, 2));
        // A deleted upstream reports neither count.
        assert_eq!(parse_track("[gone]"), (0, 0));
    }

    #[test]
    fn rejects_a_record_with_missing_fields() {
        assert!(parse(&line(&["refs/heads/main", "aaa"])).is_err());
    }

    #[test]
    fn an_empty_listing_is_not_an_error() {
        assert!(parse(b"").unwrap().is_empty());
    }
}
