//! Whether a proposal's latest commit passes its checks: GitHub's check runs and commit
//! statuses, or the newest GitLab pipeline for it, put in one shape.

use serde::Deserialize;

use crate::provider::HostingError;

/// Where a commit's checks stand, worst first: one failure is a failure whatever else passed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckState {
    Failing,
    Running,
    Passing,
    /// Nothing ran, or nothing that counts.
    None,
}

/// What a commit's checks came to.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Checks {
    pub state: CheckState,
    /// The whole of it in a few words, such as "3 of 4 checks passed".
    pub summary: String,
    /// Where to see them: the first failure, or else the first still running.
    pub url: Option<String>,
}

/// One check, reduced to what the summary needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Passed,
    Failed,
    Running,
    /// Skipped or neutral: it ran, or chose not to, and has nothing to say about the commit.
    Ignored,
}

/// Whether `sha` can go into a URL as it is: forty hex digits, or sixty-four for SHA-256.
#[must_use]
pub fn is_commit_id(sha: &str) -> bool {
    matches!(sha.len(), 40 | 64) && sha.bytes().all(|b| b.is_ascii_hexdigit())
}

fn summarise(checks: &[(Outcome, Option<String>)], noun: &str) -> Checks {
    let count = |o: Outcome| checks.iter().filter(|(c, _)| *c == o).count();
    let (passed, failed, running) = (
        count(Outcome::Passed),
        count(Outcome::Failed),
        count(Outcome::Running),
    );
    let counted = passed + failed + running;
    let first = |o: Outcome| {
        checks
            .iter()
            .find(|(c, _)| *c == o)
            .and_then(|(_, u)| u.clone())
    };
    let plural = |n: usize| {
        if n == 1 {
            noun.to_owned()
        } else {
            format!("{noun}s")
        }
    };
    if failed > 0 {
        return Checks {
            state: CheckState::Failing,
            summary: format!("{failed} of {counted} {} failed", plural(counted)),
            url: first(Outcome::Failed),
        };
    }
    if running > 0 {
        return Checks {
            state: CheckState::Running,
            summary: format!("{running} of {counted} {} still running", plural(counted)),
            url: first(Outcome::Running),
        };
    }
    if passed > 0 {
        let summary = if passed == 1 {
            format!("the {noun} passed")
        } else {
            format!("all {passed} {} passed", plural(passed))
        };
        return Checks {
            state: CheckState::Passing,
            summary,
            url: None,
        };
    }
    Checks {
        state: CheckState::None,
        summary: format!("no {}", plural(2)),
        url: None,
    }
}

#[derive(Deserialize)]
struct CheckRuns {
    check_runs: Vec<CheckRun>,
}

#[derive(Deserialize)]
struct CheckRun {
    status: String,
    #[serde(default)]
    conclusion: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
}

#[derive(Deserialize)]
struct Combined {
    statuses: Vec<Status>,
}

#[derive(Deserialize)]
struct Status {
    state: String,
    #[serde(default)]
    target_url: Option<String>,
}

/// GitHub's two kinds together: check runs, which Actions and most apps report, and commit
/// statuses, which older integrations still post.
/// <https://docs.github.com/en/rest/checks/runs#list-check-runs-for-a-git-reference>
/// <https://docs.github.com/en/rest/commits/statuses#get-the-combined-status-for-a-specific-reference>
///
/// # Errors
/// [`HostingError::Malformed`] when either body is not what its endpoint documents.
pub fn parse_github(check_runs: &[u8], statuses: &[u8]) -> Result<Checks, HostingError> {
    let runs: CheckRuns =
        serde_json::from_slice(check_runs).map_err(|e| HostingError::Malformed(e.to_string()))?;
    let combined: Combined =
        serde_json::from_slice(statuses).map_err(|e| HostingError::Malformed(e.to_string()))?;
    let mut all: Vec<(Outcome, Option<String>)> = runs
        .check_runs
        .into_iter()
        .map(|r| {
            let outcome = if r.status == "completed" {
                match r.conclusion.as_deref() {
                    Some("success") => Outcome::Passed,
                    Some("neutral" | "skipped") => Outcome::Ignored,
                    _ => Outcome::Failed,
                }
            } else {
                Outcome::Running
            };
            (outcome, r.html_url)
        })
        .collect();
    all.extend(combined.statuses.into_iter().map(|s| {
        let outcome = match s.state.as_str() {
            "success" => Outcome::Passed,
            "pending" => Outcome::Running,
            _ => Outcome::Failed,
        };
        (outcome, s.target_url)
    }));
    Ok(summarise(&all, "check"))
}

#[derive(Deserialize)]
struct Pipeline {
    status: String,
    #[serde(default)]
    web_url: Option<String>,
}

/// The newest GitLab pipeline for the commit, from a list asked for newest first.
/// <https://docs.gitlab.com/api/pipelines/#list-project-pipelines>
///
/// # Errors
/// [`HostingError::Malformed`] when the body is not a list of pipelines.
pub fn parse_gitlab(pipelines: &[u8]) -> Result<Checks, HostingError> {
    let list: Vec<Pipeline> =
        serde_json::from_slice(pipelines).map_err(|e| HostingError::Malformed(e.to_string()))?;
    let found: Vec<(Outcome, Option<String>)> = list
        .into_iter()
        .take(1)
        .map(|p| {
            let outcome = match p.status.as_str() {
                "success" => Outcome::Passed,
                "failed" | "canceled" => Outcome::Failed,
                "skipped" | "manual" => Outcome::Ignored,
                // created, waiting_for_resource, preparing, pending, running, scheduled
                _ => Outcome::Running,
            };
            (outcome, p.web_url)
        })
        .collect();
    Ok(summarise(&found, "pipeline"))
}

#[cfg(test)]
mod tests {
    use super::{CheckState, is_commit_id, parse_github, parse_gitlab};

    const NO_STATUSES: &[u8] = br#"{"state":"pending","statuses":[],"total_count":0}"#;

    fn runs(list: &str) -> Vec<u8> {
        format!(r#"{{"total_count":0,"check_runs":[{list}]}}"#).into_bytes()
    }

    #[test]
    fn one_failure_fails_the_commit_and_is_where_it_points() {
        let body = runs(
            r#"{"status":"completed","conclusion":"success","html_url":"https://x/1"},
               {"status":"completed","conclusion":"failure","html_url":"https://x/2"},
               {"status":"in_progress","conclusion":null,"html_url":"https://x/3"}"#,
        );
        let checks = parse_github(&body, NO_STATUSES).unwrap();
        assert_eq!(checks.state, CheckState::Failing);
        assert_eq!(checks.summary, "1 of 3 checks failed");
        assert_eq!(checks.url.as_deref(), Some("https://x/2"));
    }

    #[test]
    fn running_until_every_check_has_finished() {
        let body = runs(
            r#"{"status":"completed","conclusion":"success"},
               {"status":"queued","conclusion":null,"html_url":"https://x/q"}"#,
        );
        let checks = parse_github(&body, NO_STATUSES).unwrap();
        assert_eq!(checks.state, CheckState::Running);
        assert_eq!(checks.summary, "1 of 2 checks still running");
    }

    #[test]
    fn skipped_and_neutral_checks_say_nothing_either_way() {
        let body = runs(
            r#"{"status":"completed","conclusion":"success"},
               {"status":"completed","conclusion":"skipped"},
               {"status":"completed","conclusion":"neutral"}"#,
        );
        let checks = parse_github(&body, NO_STATUSES).unwrap();
        assert_eq!(checks.state, CheckState::Passing);
        assert_eq!(checks.summary, "the check passed");
    }

    #[test]
    fn a_cancelled_or_timed_out_check_is_a_failure() {
        for conclusion in [
            "cancelled",
            "timed_out",
            "action_required",
            "startup_failure",
        ] {
            let body = runs(&format!(
                r#"{{"status":"completed","conclusion":"{conclusion}"}}"#
            ));
            assert_eq!(
                parse_github(&body, NO_STATUSES).unwrap().state,
                CheckState::Failing
            );
        }
    }

    #[test]
    fn commit_statuses_count_beside_check_runs() {
        let statuses = br#"{"state":"failure","statuses":[
            {"state":"success","target_url":"https://ci/1"},
            {"state":"error","target_url":"https://ci/2"}]}"#;
        let body = runs(r#"{"status":"completed","conclusion":"success"}"#);
        let checks = parse_github(&body, statuses).unwrap();
        assert_eq!(checks.state, CheckState::Failing);
        assert_eq!(checks.summary, "1 of 3 checks failed");
        assert_eq!(checks.url.as_deref(), Some("https://ci/2"));
    }

    #[test]
    fn nothing_ran_is_its_own_answer() {
        let checks = parse_github(&runs(""), NO_STATUSES).unwrap();
        assert_eq!(checks.state, CheckState::None);
        assert_eq!(checks.summary, "no checks");
    }

    #[test]
    fn gitlab_reads_the_newest_pipeline_only() {
        let body = br#"[{"id":9,"status":"failed","web_url":"https://gl/p/9"},
                        {"id":8,"status":"success","web_url":"https://gl/p/8"}]"#;
        let checks = parse_gitlab(body).unwrap();
        assert_eq!(checks.state, CheckState::Failing);
        assert_eq!(checks.summary, "1 of 1 pipeline failed");
        assert_eq!(checks.url.as_deref(), Some("https://gl/p/9"));
        let passed = parse_gitlab(br#"[{"status":"success"}]"#).unwrap();
        assert_eq!(passed.summary, "the pipeline passed");
        assert_eq!(
            parse_gitlab(br#"[{"status":"pending"}]"#).unwrap().state,
            CheckState::Running
        );
        assert_eq!(parse_gitlab(b"[]").unwrap().state, CheckState::None);
    }

    #[test]
    fn refuses_a_body_that_is_not_the_documented_one() {
        assert!(parse_github(b"{}", NO_STATUSES).is_err());
        assert!(parse_gitlab(b"{}").is_err());
    }

    #[test]
    fn only_a_commit_id_goes_into_the_url() {
        assert!(is_commit_id(&"a".repeat(40)));
        assert!(is_commit_id(&"0".repeat(64)));
        assert!(!is_commit_id("main"));
        assert!(!is_commit_id(&format!("{}/../x", "a".repeat(35))));
    }
}
