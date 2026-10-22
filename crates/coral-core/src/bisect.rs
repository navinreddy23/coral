//! Finding the commit that broke something, by halving the suspects until one is left.
//!
//! git keeps the whole of a bisection in the repository: the verdicts as refs under
//! `refs/bisect/`, the log in `BISECT_LOG`, and the words used for the two sides in
//! `BISECT_TERMS`. Reading those rather than remembering anything here means a bisection
//! started in a terminal is shown and carried on like one started in the window.
//! <https://git-scm.com/docs/git-bisect>

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;

/// What a commit was found to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    /// Has the fault.
    Bad,
    /// Does not.
    Good,
    /// Cannot be tested, because it does not build, say. git picks a neighbour instead.
    Skip,
}

/// The words a bisection calls its two sides.
///
/// `bad` and `good` unless it was started with `--term-new`/`--term-old`, or as `new`/`old`,
/// which is the same search under names that fit a change that is not a fault.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Terms {
    pub bad: String,
    pub good: String,
}

impl Default for Terms {
    fn default() -> Self {
        Self {
            bad: "bad".to_owned(),
            good: "good".to_owned(),
        }
    }
}

impl Terms {
    /// Reads `BISECT_TERMS`: the bad term on the first line, the good one on the second.
    #[must_use]
    pub fn parse(file: &str) -> Self {
        let mut lines = file.lines().map(str::trim).filter(|l| !l.is_empty());
        match (lines.next(), lines.next()) {
            (Some(bad), Some(good)) => Self {
                bad: bad.to_owned(),
                good: good.to_owned(),
            },
            _ => Self::default(),
        }
    }

    fn word(&self, verdict: Verdict) -> &str {
        match verdict {
            Verdict::Bad => &self.bad,
            Verdict::Good => &self.good,
            Verdict::Skip => "skip",
        }
    }
}

/// Where a bisection has got to.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Bisection {
    pub terms: Terms,
    /// The newest commit marked bad. git keeps only one: a second bad mark replaces it.
    pub bad: Option<String>,
    pub good: Vec<String>,
    pub skipped: Vec<String>,
    /// The commit checked out to be tested next, once both sides are known.
    pub testing: Option<String>,
    /// How many commits could still be the first bad one, the bad one included.
    pub suspects: Option<u32>,
    /// Roughly how many more verdicts it will take.
    pub steps: Option<u32>,
    /// The first bad commit, once it is the only suspect left.
    pub found: Option<String>,
    /// True when every suspect but the bad commit was skipped, so git cannot say which of
    /// them it was.
    pub only_skipped_left: bool,
    /// True when a commit marked good is not an ancestor of the one marked bad, which is what
    /// marking the two the wrong way round leaves. git records both and then cannot narrow
    /// anything; ending the bisection is the way out.
    pub mistaken: bool,
    /// Every verdict given, oldest first. The refs keep only the newest bad commit, so this is
    /// what can say that an earlier one was judged bad too.
    pub verdicts: Vec<Judged>,
}

/// One verdict on one commit, as `BISECT_LOG` records it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Judged {
    pub oid: String,
    pub verdict: Verdict,
}

/// Reads the verdicts out of `BISECT_LOG`: its `git bisect <term> <oid>…` lines.
///
/// The log is a replayable script, so these lines are what git itself would run again. The
/// comment lines beside them repeat the same verdicts with subjects and are not needed.
#[must_use]
pub fn parse_log(log: &str, terms: &Terms) -> Vec<Judged> {
    let mut out = Vec::new();
    for line in log.lines() {
        let Some(rest) = line.strip_prefix("git bisect ") else {
            continue;
        };
        let mut words = rest.split_whitespace();
        let verdict = match words.next() {
            Some(w) if w == terms.bad => Verdict::Bad,
            Some(w) if w == terms.good => Verdict::Good,
            Some("skip") => Verdict::Skip,
            _ => continue,
        };
        out.extend(words.map(|oid| Judged {
            oid: oid.to_owned(),
            verdict,
        }));
    }
    out
}

/// The verdicts recorded under `refs/bisect/`, from `for-each-ref --format=%(refname) %(objectname)`.
///
/// # Errors
/// [`CoralError::Protocol`] for a line that is not a name and an object id.
pub fn parse_refs(input: &[u8], terms: &Terms) -> Result<Bisection, CoralError> {
    let mut out = Bisection {
        terms: terms.clone(),
        ..Bisection::default()
    };
    let text = String::from_utf8_lossy(input);
    for line in text.lines().filter(|l| !l.is_empty()) {
        let (name, oid) = line.split_once(' ').ok_or_else(|| CoralError::Protocol {
            label: "bisect refs",
            detail: format!("expected a ref name and an object id, got {line:?}"),
        })?;
        let Some(rest) = name.strip_prefix("refs/bisect/") else {
            continue;
        };
        let oid = oid.trim().to_owned();
        if rest == terms.bad {
            out.bad = Some(oid);
        } else if rest
            .strip_prefix(&terms.good)
            .is_some_and(|r| r.starts_with('-'))
        {
            out.good.push(oid);
        } else if rest.starts_with("skip-") {
            out.skipped.push(oid);
        }
    }
    Ok(out)
}

/// What `rev-list --bisect-vars` says about the suspects.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Vars {
    /// The commit git would test next.
    pub next: Option<String>,
    /// Every suspect, the bad commit included.
    pub all: u32,
    pub steps: u32,
}

/// Reads `bisect_rev='…'`, `bisect_all=…` and `bisect_steps=…` from `rev-list --bisect-vars`.
///
/// # Errors
/// [`CoralError::Protocol`] when the counts are missing or are not numbers.
pub fn parse_vars(input: &[u8]) -> Result<Vars, CoralError> {
    let text = String::from_utf8_lossy(input);
    let mut next = None;
    let mut all = None;
    let mut steps = None;
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('\'');
        match key.trim() {
            "bisect_rev" if !value.is_empty() => next = Some(value.to_owned()),
            "bisect_all" => all = value.parse().ok(),
            "bisect_steps" => steps = value.parse().ok(),
            _ => {}
        }
    }
    match (all, steps) {
        (Some(all), Some(steps)) => Ok(Vars { next, all, steps }),
        _ => Err(CoralError::Protocol {
            label: "bisect vars",
            detail: format!("no counts in {text:?}"),
        }),
    }
}

impl RepoLocation {
    /// The bisection in progress, or `None` when there is none.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn bisection(&self, runner: &GitRunner) -> Result<Option<Bisection>, CoralError> {
        if !self.git_path("BISECT_START").exists() {
            return Ok(None);
        }
        let terms = std::fs::read_to_string(self.git_path("BISECT_TERMS"))
            .map(|t| Terms::parse(&t))
            .unwrap_or_default();
        let refs = runner
            .output(
                GitCommand::read("for-each-ref", self.display_path())
                    .args(["for-each-ref", "--format=%(refname) %(objectname)"])
                    .arg("refs/bisect/"),
            )
            .await?;
        let mut state = parse_refs(&refs.stdout, &terms)?;
        state.verdicts = std::fs::read_to_string(self.git_path("BISECT_LOG"))
            .map(|log| parse_log(&log, &terms))
            .unwrap_or_default();
        if let Some(bad) = state.bad.clone()
            && !state.good.is_empty()
        {
            self.count_suspects(runner, &mut state, &bad).await?;
        }
        Ok(Some(state))
    }

    /// Counts the suspects between the bad commit and the good ones, and names the culprit
    /// once there is only one.
    async fn count_suspects(
        &self,
        runner: &GitRunner,
        state: &mut Bisection,
        bad: &str,
    ) -> Result<(), CoralError> {
        // Exit 1 with nothing printed is rev-list's answer when a good commit is not below the
        // bad one, so there is nothing between them to count.
        let vars = runner
            .output_allowing(
                GitCommand::read("rev-list", self.display_path())
                    .args(["rev-list", "--bisect-vars", bad, "--not"])
                    .args(&state.good),
                &[1],
            )
            .await?;
        let vars = match parse_vars(&vars.stdout) {
            Ok(vars) if vars.all > 0 && self.good_below_bad(runner, state, bad).await => vars,
            _ => {
                state.mistaken = true;
                return Ok(());
            }
        };
        state.suspects = Some(vars.all);
        state.steps = Some(vars.steps);
        if vars.all <= 1 {
            state.found = Some(bad.to_owned());
            return Ok(());
        }
        // A few suspects, all skipped: only then is the list worth reading, and it is short.
        if usize::try_from(vars.all - 1).is_ok_and(|n| n <= state.skipped.len()) {
            let list = runner
                .output(
                    GitCommand::read("rev-list", self.display_path())
                        .args(["rev-list", bad, "--not"])
                        .args(&state.good),
                )
                .await?;
            let text = String::from_utf8_lossy(&list.stdout);
            state.only_skipped_left = text
                .split_whitespace()
                .filter(|oid| *oid != bad)
                .all(|oid| state.skipped.iter().any(|s| s == oid));
        }
        if !state.only_skipped_left {
            state.testing = self.rev_parse(runner, "HEAD").await.ok();
        }
        Ok(())
    }

    /// Whether every good commit is an ancestor of the bad one, as a bisection needs.
    ///
    /// rev-list alone does not say: a good commit that descends from the bad one excludes it
    /// and everything under it, which can leave a count that looks like an answer.
    async fn good_below_bad(&self, runner: &GitRunner, state: &Bisection, bad: &str) -> bool {
        for good in &state.good {
            let out = runner
                .output_allowing(
                    GitCommand::read("merge-base", self.display_path())
                        .args(["merge-base", "--is-ancestor"])
                        .arg(good)
                        .arg(bad),
                    &[1],
                )
                .await;
            if !matches!(out, Ok(o) if o.code == 0) {
                return false;
            }
        }
        true
    }

    /// Records a verdict on `rev`, or on the commit checked out when `rev` is `None`,
    /// starting a bisection first if there is none.
    ///
    /// Once both a bad and a good commit are known, git checks out the next one to test, so
    /// this moves HEAD.
    ///
    /// # Errors
    /// Propagates git failures, such as local changes the next checkout would overwrite.
    pub async fn bisect_mark(
        &self,
        runner: &GitRunner,
        verdict: Verdict,
        rev: Option<&str>,
    ) -> Result<Option<Bisection>, CoralError> {
        if !self.git_path("BISECT_START").exists() {
            runner
                .output(GitCommand::write("bisect", self.display_path()).args(["bisect", "start"]))
                .await?;
        }
        let terms = std::fs::read_to_string(self.git_path("BISECT_TERMS"))
            .map(|t| Terms::parse(&t))
            .unwrap_or_default();
        let mut cmd = GitCommand::write("bisect", self.display_path())
            .arg("bisect")
            .arg(terms.word(verdict));
        if let Some(rev) = rev {
            cmd = cmd.arg(rev);
        }
        let marked = runner.output(cmd).await;
        let state = self.bisection(runner).await?;
        match marked {
            // git's own words for this are on stdout, so the error carries nothing worth
            // showing; this says what happened and what to do.
            Err(_) if state.as_ref().is_some_and(|s| s.mistaken) => Err(CoralError::Refused {
                label: "bisect",
                detail: format!(
                    "a commit marked {} is newer than the one marked {}, as if the two were \
                     swapped; end the bisect and start again",
                    state.as_ref().map_or("good", |s| s.terms.good.as_str()),
                    state.as_ref().map_or("bad", |s| s.terms.bad.as_str()),
                ),
            }),
            Ok(_) => Ok(state),
            // git records the verdict and then exits 2 when every suspect left was skipped.
            // That is an answer, which the state says, not a failure to mark.
            Err(CoralError::GitExit { code: 2, .. })
                if state.as_ref().is_some_and(|s| s.only_skipped_left) =>
            {
                Ok(state)
            }
            Err(e) => Err(e),
        }
    }

    /// Ends the bisection and puts HEAD back where it was before it started.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn bisect_reset(&self, runner: &GitRunner) -> Result<(), CoralError> {
        runner
            .output(GitCommand::write("bisect", self.display_path()).args(["bisect", "reset"]))
            .await
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::{Judged, Terms, Verdict, parse_log, parse_refs, parse_vars};

    #[test]
    fn reads_every_verdict_from_the_log() {
        let log = "git bisect start\n\
                   # status: waiting for both good and bad commits\n\
                   # bad: [888] step 8\n\
                   git bisect bad 888\n\
                   git bisect good 111\n\
                   git bisect bad 666\n\
                   git bisect skip 333 444\n\
                   # first bad commit: [555] step 5\n";
        let judged = parse_log(log, &Terms::default());
        let pairs: Vec<(&str, Verdict)> =
            judged.iter().map(|j| (j.oid.as_str(), j.verdict)).collect();
        assert_eq!(
            pairs,
            [
                ("888", Verdict::Bad),
                ("111", Verdict::Good),
                ("666", Verdict::Bad),
                ("333", Verdict::Skip),
                ("444", Verdict::Skip),
            ]
        );
    }

    #[test]
    fn reads_the_log_in_its_own_terms() {
        let terms = Terms {
            bad: "new".into(),
            good: "old".into(),
        };
        let judged = parse_log(
            "git bisect new 9\ngit bisect old 1\ngit bisect bad 5\n",
            &terms,
        );
        assert_eq!(
            judged,
            [
                Judged {
                    oid: "9".into(),
                    verdict: Verdict::Bad
                },
                Judged {
                    oid: "1".into(),
                    verdict: Verdict::Good
                },
            ]
        );
    }

    #[test]
    fn reads_the_terms_or_falls_back_to_bad_and_good() {
        assert_eq!(
            Terms::parse("new\nold\n"),
            Terms {
                bad: "new".into(),
                good: "old".into()
            }
        );
        assert_eq!(Terms::parse(""), Terms::default());
        assert_eq!(Terms::parse("only-one\n"), Terms::default());
    }

    #[test]
    fn sorts_the_verdicts_by_their_ref() {
        let input = b"refs/bisect/bad bbb\n\
                      refs/bisect/good-111 111\n\
                      refs/bisect/good-222 222\n\
                      refs/bisect/skip-333 333\n";
        let state = parse_refs(input, &Terms::default()).unwrap();
        assert_eq!(state.bad.as_deref(), Some("bbb"));
        assert_eq!(state.good, ["111", "222"]);
        assert_eq!(state.skipped, ["333"]);
    }

    #[test]
    fn reads_the_verdicts_under_other_terms() {
        let terms = Terms {
            bad: "new".into(),
            good: "old".into(),
        };
        let input = b"refs/bisect/new nnn\nrefs/bisect/old-111 111\n";
        let state = parse_refs(input, &terms).unwrap();
        assert_eq!(state.bad.as_deref(), Some("nnn"));
        assert_eq!(state.good, ["111"]);
    }

    #[test]
    fn a_term_that_begins_another_is_not_mistaken_for_it() {
        // "good" must not claim refs/bisect/goodness, which is not one of git's.
        let input = b"refs/bisect/goodness xxx\nrefs/bisect/good-1 1\n";
        let state = parse_refs(input, &Terms::default()).unwrap();
        assert_eq!(state.good, ["1"]);
    }

    #[test]
    fn nothing_marked_yet_is_an_empty_bisection() {
        let state = parse_refs(b"", &Terms::default()).unwrap();
        assert_eq!(state.bad, None);
        assert_eq!(state.good, Vec::<String>::new());
    }

    #[test]
    fn refuses_a_line_it_cannot_split() {
        assert!(parse_refs(b"refs/bisect/bad\n", &Terms::default()).is_err());
    }

    #[test]
    fn reads_the_counts_rev_list_gives() {
        let input = b"bisect_rev='abc123'\nbisect_nr=3\nbisect_good=3\nbisect_bad=4\n\
                      bisect_all=8\nbisect_steps=2\n";
        let vars = parse_vars(input).unwrap();
        assert_eq!(vars.next.as_deref(), Some("abc123"));
        assert_eq!(vars.all, 8);
        assert_eq!(vars.steps, 2);
    }

    #[test]
    fn the_last_suspect_has_no_next_commit() {
        let vars = parse_vars(b"bisect_rev=''\nbisect_all=1\nbisect_steps=0\n").unwrap();
        assert_eq!(vars.next, None);
        assert_eq!(vars.all, 1);
    }

    #[test]
    fn refuses_output_with_no_counts() {
        assert!(parse_vars(b"").is_err());
        assert!(parse_vars(b"bisect_all=many\nbisect_steps=2\n").is_err());
    }
}
