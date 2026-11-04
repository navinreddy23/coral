//! An issue a commit message or a branch refers to, read from the host that tracks it.

/// Whether an issue is still to be done.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IssueState {
    Open,
    Closed,
}

/// One issue, or work item on Azure DevOps, reduced to what a reference needs.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub state: IssueState,
    /// Where a person would open it.
    pub web_url: String,
}

/// A branch name made from an issue: `123-the-first-few-words`, lower case, hyphens between
/// words, nothing git would refuse and nothing longer than a branch list can show.
#[must_use]
pub fn branch_name(issue: &Issue) -> String {
    let mut words = Vec::new();
    let mut length = 0;
    for word in issue
        .title
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        if length + word.len() > 40 {
            break;
        }
        length += word.len() + 1;
        words.push(word.to_ascii_lowercase());
    }
    if words.is_empty() {
        return format!("issue-{}", issue.number);
    }
    format!("{}-{}", issue.number, words.join("-"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titled(title: &str) -> Issue {
        Issue {
            number: 123,
            title: title.to_owned(),
            state: IssueState::Open,
            web_url: String::new(),
        }
    }

    #[test]
    fn a_branch_is_named_for_the_issue_and_stays_short_and_legal() {
        assert_eq!(
            branch_name(&titled("Crash when the remote's HEAD is unborn!")),
            "123-crash-when-the-remote-s-head-is-unborn"
        );
        assert_eq!(branch_name(&titled("…")), "issue-123");
        let long = branch_name(&titled(&"word ".repeat(40)));
        assert!(long.len() <= 50, "{long}");
        assert!(!long.ends_with('-'));
    }
}
