//! The commit message template a repository asks for, in `commit.template`.
//!
//! Git puts it in the editor for a commit made without `-m`. Coral has no editor, so the
//! template is what the message box starts from instead, and the same rule applies: a message
//! left exactly as the template is no message (see `git help commit`, `--template`).

use std::path::Path;

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;

/// A template's text without the `#` lines git strips from a message, or `None` when nothing
/// else is left.
#[must_use]
pub fn usable(raw: &str) -> Option<String> {
    let kept: Vec<&str> = raw
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(str::trim_end)
        .collect();
    let text = kept.join("\n").trim().to_owned();
    (!text.is_empty()).then_some(text)
}

impl RepoLocation {
    /// The template's usable text, or `None` when none is set, its file cannot be read, or it
    /// holds nothing but comments.
    ///
    /// An unreadable file is not an error here. git refuses the commit itself over one, with
    /// its own message, and that is the moment it matters; an empty box until then costs
    /// nothing.
    ///
    /// # Errors
    /// Propagates git failures other than the setting being absent.
    pub async fn commit_template(&self, runner: &GitRunner) -> Result<Option<String>, CoralError> {
        // `--path` expands a leading `~`, as git does when it reads the setting itself.
        let out = runner
            .output_allowing(
                GitCommand::read("config", self.display_path()).args([
                    "config",
                    "--path",
                    "--get",
                    "commit.template",
                ]),
                &[1],
            )
            .await?;
        let named = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        if named.is_empty() {
            return Ok(None);
        }
        // git reads a relative name from wherever it was started, and Coral starts it at the
        // top of the working tree.
        let file = match &self.workdir {
            Some(root) if Path::new(&named).is_relative() => root.join(&named),
            _ => named.into(),
        };
        Ok(std::fs::read_to_string(file)
            .ok()
            .and_then(|raw| usable(&raw)))
    }
}

#[cfg(test)]
mod tests {
    use super::usable;

    #[test]
    fn keeps_the_text_and_drops_the_comments() {
        assert_eq!(
            usable("[JIRA-] \n\n# Say why, not what.\n# Wrap at 72.\n").as_deref(),
            Some("[JIRA-]")
        );
        assert_eq!(
            usable("Subject\n\nWhy:\n# a hint\nTested:\n").as_deref(),
            Some("Subject\n\nWhy:\nTested:")
        );
    }

    #[test]
    fn a_template_of_only_comments_is_none() {
        assert_eq!(usable("# Describe the change\n#\n\n"), None);
        assert_eq!(usable(""), None);
    }
}
