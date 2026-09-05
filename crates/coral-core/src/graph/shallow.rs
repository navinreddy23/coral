//! The boundary of a shallow clone, and walking up to it without falling off the edge.

use std::collections::HashSet;

use gix::ObjectId;

/// The commits `.git/shallow` names, or an empty set when the clone is a whole one.
///
/// A failure to read the file is an empty boundary rather than an error: an unreadable
/// `shallow` file leaves a repository that walks as if it were complete, which is what every
/// version of Coral before this one did with every shallow clone.
#[must_use]
pub fn boundary(repo: &gix::Repository) -> HashSet<ObjectId> {
    repo.shallow_commits()
        .ok()
        .flatten()
        .map(|commits| commits.iter().copied().collect())
        .unwrap_or_default()
}

/// An object database whose shallow boundary commits have had their parents taken off.
///
/// `.git/shallow` lists the commits whose parents were never fetched. git reads it and treats
/// them as roots; gix's traversal does not, so it asks the database for a parent that is not
/// there and the walk fails with "an object … could not be found". That is what opening a
/// `west` workspace's modules did, since west clones them one commit deep.
///
/// Grafting is git's own answer, and this is the same thing done at the point the walker reads
/// an object: the boundary commits come back with their `parent` lines removed, so the walk
/// ends there rather than stepping into history the clone does not have. Only those commits
/// are rewritten, and there is usually one, so the copy it costs is not worth avoiding.
pub struct Grafted<F> {
    inner: F,
    boundary: HashSet<ObjectId>,
}

impl<F> Grafted<F> {
    pub fn new(inner: F, boundary: HashSet<ObjectId>) -> Self {
        Self { inner, boundary }
    }
}

impl<F: gix::objs::Find> gix::objs::Find for Grafted<F> {
    fn try_find<'a>(
        &self,
        id: &gix::hash::oid,
        buffer: &'a mut Vec<u8>,
    ) -> Result<Option<gix::objs::Data<'a>>, gix::objs::find::Error> {
        if !self.boundary.contains(id) {
            return self.inner.try_find(id, buffer);
        }
        // Read somewhere else first: the database may hand back a slice of a memory-mapped
        // pack rather than the buffer it was given, so the rewritten bytes have to be put into
        // `buffer` afterwards rather than edited where they landed.
        let mut raw = Vec::new();
        let Some(found) = self.inner.try_find(id, &mut raw)? else {
            return Ok(None);
        };
        let kind = found.kind;
        let hash = found.object_hash;
        let bytes = if kind == gix::objs::Kind::Commit {
            without_parents(found.data)
        } else {
            found.data.to_vec()
        };
        buffer.clear();
        buffer.extend_from_slice(&bytes);
        Ok(Some(gix::objs::Data::new(buffer.as_slice(), kind, hash)))
    }
}

/// A commit object with its `parent` lines dropped, which is what a graft is.
///
/// Byte surgery on the header rather than a decode and re-encode, so nothing else about the
/// commit — the exact author line, the encoding header, a signature — can come back different
/// from how it went in.
fn without_parents(commit: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(commit.len());
    let mut rest = commit;
    loop {
        let Some(end) = rest.iter().position(|b| *b == b'\n') else {
            out.extend_from_slice(rest);
            return out;
        };
        let (line, tail) = rest.split_at(end + 1);
        // The header ends at the first empty line. Everything past it is the message, where a
        // line that happens to begin with "parent " is text and must be left alone.
        if line == b"\n" {
            out.extend_from_slice(line);
            out.extend_from_slice(tail);
            return out;
        }
        if !line.starts_with(b"parent ") {
            out.extend_from_slice(line);
        }
        rest = tail;
    }
}

#[cfg(test)]
mod tests {
    use super::without_parents;

    #[test]
    fn a_merge_loses_both_parents_and_keeps_everything_else() {
        let commit = b"tree aaa\nparent bbb\nparent ccc\nauthor A <a> 1 +0000\n\nthe message\n";
        assert_eq!(
            without_parents(commit),
            b"tree aaa\nauthor A <a> 1 +0000\n\nthe message\n".to_vec()
        );
    }

    #[test]
    fn a_root_commit_is_left_exactly_as_it_was() {
        let commit = b"tree aaa\nauthor A <a> 1 +0000\n\nthe message\n";
        assert_eq!(without_parents(commit), commit.to_vec());
    }

    /// The header ends at the blank line, so a message about parents keeps its words.
    #[test]
    fn a_message_that_talks_about_parents_is_not_edited() {
        let commit =
            b"tree aaa\nparent bbb\nauthor A <a> 1 +0000\n\nparent selection\nparent two\n";
        assert_eq!(
            without_parents(commit),
            b"tree aaa\nauthor A <a> 1 +0000\n\nparent selection\nparent two\n".to_vec()
        );
    }

    #[test]
    fn a_commit_with_no_message_and_no_trailing_newline_survives() {
        let commit = b"tree aaa\nparent bbb\nauthor A <a> 1 +0000";
        assert_eq!(
            without_parents(commit),
            b"tree aaa\nauthor A <a> 1 +0000".to_vec()
        );
    }
}
