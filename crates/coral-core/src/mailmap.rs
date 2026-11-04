//! `.mailmap`: one person, under every name and address they have committed as.
//!
//! git applies it wherever it prints a name with `%aN`, and in blame and shortlog. The graph's
//! rows are read from raw objects instead, so the mapping is applied here, through gix, which
//! reads the same three sources git does: the worktree's `.mailmap` (or HEAD's, when bare),
//! `mailmap.file` and `mailmap.blob`.
//! <https://git-scm.com/docs/gitmailmap>

use std::path::Path;

use bstr::BStr;

/// The repository's mailmap, empty when it has none or it cannot be read.
#[derive(Default)]
pub struct Mailmap(gix::mailmap::Snapshot);

impl Mailmap {
    /// Reads the mailmap for the repository at `path`, its worktree or its git directory.
    ///
    /// An unreadable mailmap is no mailmap: every name is then shown as it was committed,
    /// which is what git does too.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        gix::open(path).map_or_else(|_| Self::default(), |repo| Self(repo.open_mailmap()))
    }

    /// Whether there is anything to map, so a caller can skip the work for most repositories.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.iter().next().is_none()
    }

    /// The name and address `name <email>` is known by, or `None` when it is not mapped.
    #[must_use]
    pub fn person(&self, name: &str, email: &str) -> Option<(String, String)> {
        let signature = gix::actor::SignatureRef {
            name: BStr::new(name),
            email: BStr::new(email),
            time: "",
        };
        self.0.try_resolve(signature).map(|s| {
            (
                String::from_utf8_lossy(&s.name).into_owned(),
                String::from_utf8_lossy(&s.email).into_owned(),
            )
        })
    }
}
