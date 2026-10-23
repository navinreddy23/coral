//! Adding a line to the repository's own `.gitignore`.
//!
//! Only ever appended to, never rewritten: the file is the user's, often hand-ordered and
//! commented, and a negation (`!keep.log`) only works below the pattern it undoes.

use crate::error::CoralError;
use crate::repo::RepoLocation;

/// The pattern that matches exactly one path from the repository root, and nothing else.
///
/// Anchored with a leading slash, so `out.log` at the root does not also ignore `logs/out.log`.
/// The characters `gitignore` gives a meaning to are escaped (see `git help gitignore`, under
/// "PATTERN FORMAT"): a `*` in a file name is a file name, not a wildcard.
#[must_use]
pub fn exact(path: &str) -> String {
    let mut out = String::with_capacity(path.len() + 2);
    out.push('/');
    for c in path.chars() {
        if matches!(c, '\\' | '*' | '?' | '[' | '!' | '#') {
            out.push('\\');
        }
        out.push(c);
    }
    // Trailing spaces are dropped from a pattern unless escaped.
    let kept = out.trim_end_matches(' ').len();
    let spaces = out.len() - kept;
    out.truncate(kept);
    out.push_str(&"\\ ".repeat(spaces));
    out
}

/// `existing` with `pattern` on a line of its own at the end, or `None` when it is already there.
#[must_use]
pub fn appended(existing: &str, pattern: &str) -> Option<String> {
    if existing.lines().any(|line| line.trim_end() == pattern) {
        return None;
    }
    let mut out = existing.to_owned();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(pattern);
    out.push('\n');
    Some(out)
}

impl RepoLocation {
    /// Adds `pattern` to the `.gitignore` at the top of the working tree.
    ///
    /// # Errors
    /// [`CoralError::Refused`] for a bare repository, which has no working tree to ignore files
    /// in, and I/O failures reading or writing the file.
    pub fn ignore(&self, pattern: &str) -> Result<(), CoralError> {
        let Some(root) = &self.workdir else {
            return Err(CoralError::Refused {
                label: "ignore",
                detail: "a bare repository has no working files to ignore".to_owned(),
            });
        };
        let file = root.join(".gitignore");
        let existing = match std::fs::read_to_string(&file) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e.into()),
        };
        if let Some(text) = appended(&existing, pattern) {
            std::fs::write(&file, text)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{appended, exact};

    #[test]
    fn anchors_a_path_to_the_root() {
        assert_eq!(exact("out.log"), "/out.log");
        assert_eq!(exact("build/out.log"), "/build/out.log");
    }

    #[test]
    fn escapes_what_gitignore_would_read_as_a_pattern() {
        assert_eq!(exact("a*b?.txt"), "/a\\*b\\?.txt");
        assert_eq!(exact("[draft].md"), "/\\[draft].md");
        assert_eq!(exact("!important"), "/\\!important");
        assert_eq!(exact("#notes"), "/\\#notes");
        assert_eq!(exact("odd name "), "/odd name\\ ");
    }

    #[test]
    fn appends_on_a_line_of_its_own() {
        assert_eq!(appended("", "*.log").as_deref(), Some("*.log\n"));
        assert_eq!(
            appended("target/\n", "*.log").as_deref(),
            Some("target/\n*.log\n")
        );
        assert_eq!(
            appended("target/", "*.log").as_deref(),
            Some("target/\n*.log\n")
        );
    }

    #[test]
    fn leaves_a_pattern_that_is_already_there() {
        assert_eq!(appended("# logs\n*.log\n", "*.log"), None);
    }
}
