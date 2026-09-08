use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;

/// Where a revision stands relative to `HEAD`, which is what decides the direction of every
/// operation offered on it.
///
/// Answered by ancestry rather than by counting the commits between the two. On Linux, asking
/// how far apart the tip and a twenty-year-old tag are takes two and a half seconds, and
/// asking whether one contains the other takes a seventh of one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub enum Ancestry {
    /// The commit `HEAD` is already on.
    Same,
    /// `HEAD` contains it. Nothing of it is missing here, so there is nothing to merge and no
    /// way to fast-forward to it; what can be done is move *it* up to `HEAD`.
    Behind,
    /// It contains `HEAD`, so `HEAD` can fast-forward to it.
    Ahead,
    /// Neither contains the other. A merge or a rebase, never a fast-forward.
    Diverged,
}

impl RepoLocation {
    /// Where `rev` stands relative to `HEAD`.
    ///
    /// # Errors
    /// Propagates git failures, including an unknown revision.
    pub async fn ancestry(&self, runner: &GitRunner, rev: &str) -> Result<Ancestry, CoralError> {
        let contained = self.is_ancestor(runner, rev, "HEAD").await?;
        let contains = self.is_ancestor(runner, "HEAD", rev).await?;
        Ok(match (contained, contains) {
            (true, true) => Ancestry::Same,
            (true, false) => Ancestry::Behind,
            (false, true) => Ancestry::Ahead,
            (false, false) => Ancestry::Diverged,
        })
    }

    /// Whether `older` is reachable from `newer`.
    ///
    /// `merge-base --is-ancestor` writes nothing and answers by exiting 0 or 1, so 1 has to be
    /// allowed through as a result rather than raised as a failure.
    async fn is_ancestor(
        &self,
        runner: &GitRunner,
        older: &str,
        newer: &str,
    ) -> Result<bool, CoralError> {
        let out = runner
            .output_allowing(
                GitCommand::read("merge-base", self.display_path())
                    .args(["merge-base", "--is-ancestor"])
                    // `^{commit}` so an annotated tag is compared by the commit it points at
                    // rather than by the tag object, which is an ancestor of nothing.
                    .arg(format!("{older}^{{commit}}"))
                    .arg(format!("{newer}^{{commit}}")),
                &[1],
            )
            .await?;
        Ok(out.code == 0)
    }
}

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
        .filter(|r| !matches!(r, Ok(g) if is_remote_head(&g.name)))
        .collect()
}

/// Whether a name is a remote's own `HEAD`, which is not a branch.
///
/// `refs/remotes/origin/HEAD` is a symbolic ref at whichever branch that remote calls its
/// default, so it always points at a ref this list already carries. Drawn as a branch it is a
/// second label on a commit that already has one, saying nothing the first did not — and with
/// two remotes configured it is two of them.
fn is_remote_head(name: &str) -> bool {
    name.strip_prefix("refs/remotes/")
        .and_then(|rest| rest.split_once('/'))
        .is_some_and(|(_, branch)| branch == "HEAD")
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

#[cfg(test)]
mod head_tests {
    use super::{FORMAT, parse};

    fn line(fields: &[&str]) -> Vec<u8> {
        fields.join("\0").into_bytes()
    }

    #[test]
    fn a_remote_s_own_head_is_not_listed_as_a_branch() {
        let _ = FORMAT;
        let mut input = line(&["refs/remotes/origin/main", "aaa", "commit", "", "", ""]);
        input.push(b'\n');
        input.extend(line(&[
            "refs/remotes/origin/HEAD",
            "aaa",
            "commit",
            "",
            "",
            "",
        ]));
        input.push(b'\n');
        input.extend(line(&[
            "refs/remotes/github/HEAD",
            "aaa",
            "commit",
            "",
            "",
            "",
        ]));

        let refs = parse(&input).expect("parses");
        let names: Vec<&str> = refs.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["refs/remotes/origin/main"]);
    }

    /// A branch that merely ends in HEAD is a branch. Only the remote's own is dropped.
    #[test]
    fn a_branch_whose_name_ends_in_head_is_kept() {
        let mut input = line(&[
            "refs/remotes/origin/spike/HEAD",
            "aaa",
            "commit",
            "",
            "",
            "",
        ]);
        input.push(b'\n');
        input.extend(line(&[
            "refs/heads/HEAD-first",
            "bbb",
            "commit",
            "",
            "",
            "",
        ]));

        let refs = parse(&input).expect("parses");
        assert_eq!(refs.len(), 2, "only a remote's own HEAD goes");
    }
}
