//! What the find box was asked: words to look for, and qualifiers that narrow where.
//!
//! `code:` searches what each commit changed rather than what it says about itself, and
//! `after:` and `before:` bound when it was made. Everything else is the free text the
//! message, author and path passes already answer. A word with a colon that is not one of the
//! three stays text, because commit subjects are full of them: `ui:`, `fix:`, `smb/client:`.
//! Like the rest of the box, `code:` ignores case.

use crate::error::CoralError;

/// A parsed find-box query.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Search {
    /// The words left once the qualifiers are taken out, joined by single spaces.
    pub text: String,
    pub change: Option<Change>,
    /// A date git reads, as written: `2024-05-01`, or `2.weeks` for two weeks ago.
    pub after: Option<String>,
    pub before: Option<String>,
}

/// What a commit's diff has to contain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// The string, as `-S`: commits that changed how often it occurs, which are the ones that
    /// introduced or deleted it, not every one whose diff happens to show it.
    Text(String),
    /// A line matching this extended regular expression was added or removed, as `-G`.
    Pattern(String),
}

impl Search {
    /// Reads a query.
    ///
    /// A qualifier with nothing after its colon is left as text, so a query being typed is
    /// never an error half way through the word.
    ///
    /// # Errors
    /// [`CoralError::Refused`] when a date is not one git would read the way it was meant.
    pub fn parse(query: &str) -> Result<Self, CoralError> {
        let mut out = Self::default();
        let mut words = Vec::new();
        for token in tokens(query) {
            let Some((key, value)) = token.split_once(':') else {
                words.push(unquoted(&token));
                continue;
            };
            let value = unquoted(value);
            if value.is_empty() {
                words.push(unquoted(&token));
                continue;
            }
            match key.to_ascii_lowercase().as_str() {
                "code" => out.change = Some(change(value)),
                "after" => out.after = Some(date("after", value)?),
                "before" => out.before = Some(date("before", value)?),
                _ => words.push(unquoted(&token)),
            }
        }
        out.text = words.join(" ");
        Ok(out)
    }

    /// Whether anything narrows the search beyond the free text.
    #[must_use]
    pub fn is_qualified(&self) -> bool {
        self.change.is_some() || self.after.is_some() || self.before.is_some()
    }

    /// The `git log` arguments the qualifiers add, each one whole, so a value is never read
    /// as an option of its own.
    #[must_use]
    pub fn log_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        match &self.change {
            Some(Change::Text(s)) => args.push(format!("-S{s}")),
            Some(Change::Pattern(p)) => args.push(format!("-G{p}")),
            None => {}
        }
        // Not `--since`: that stops the walk at the first commit older than the date, and a
        // rebased or cherry-picked commit carries an old date in the middle of new history.
        // https://git-scm.com/docs/git-log#Documentation/git-log.txt---since-as-filterltdategt
        if let Some(d) = &self.after {
            args.push(format!("--since-as-filter={d}"));
        }
        if let Some(d) = &self.before {
            args.push(format!("--until={d}"));
        }
        args
    }
}

/// Whitespace-separated words, where a double-quoted run is one word whatever it holds.
fn tokens(query: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    for ch in query.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                word.push(ch);
            }
            c if c.is_whitespace() && !quoted => {
                if !word.is_empty() {
                    out.push(std::mem::take(&mut word));
                }
            }
            c => word.push(c),
        }
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

fn unquoted(word: &str) -> String {
    word.replace('"', "")
}

/// `/…/` is a pattern; anything else is the literal string.
fn change(value: String) -> Change {
    match value.strip_prefix('/').and_then(|v| v.strip_suffix('/')) {
        Some(pattern) if !pattern.is_empty() => Change::Pattern(pattern.to_owned()),
        _ => Change::Text(value),
    }
}

/// A calendar date or a count of days, weeks, months or years back.
///
/// Checked here because git reads almost anything as a date without complaint, and what it
/// makes of a typo is usually "now", which filters out every commit and looks like a search
/// that found nothing.
fn date(key: &str, value: String) -> Result<String, CoralError> {
    if is_calendar_date(&value) || is_relative_date(&value) {
        Ok(value)
    } else {
        Err(CoralError::Refused {
            label: "search by date",
            detail: format!("{key}:{value} is not a date; write {key}:2024-05-01 or {key}:2.weeks"),
        })
    }
}

fn is_calendar_date(value: &str) -> bool {
    let parts: Vec<&str> = value.split('-').collect();
    let [y, m, d] = parts.as_slice() else {
        return false;
    };
    let number = |s: &str, len: usize, max: u32| {
        s.len() == len
            && s.bytes().all(|b| b.is_ascii_digit())
            && s.parse::<u32>().is_ok_and(|n| (1..=max).contains(&n))
    };
    number(y, 4, 9999) && number(m, 2, 12) && number(d, 2, 31)
}

fn is_relative_date(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if matches!(lower.as_str(), "yesterday" | "today") {
        return true;
    }
    let digits = lower.bytes().take_while(u8::is_ascii_digit).count();
    let (count, unit) = lower.split_at(digits);
    let unit = unit.strip_prefix('.').unwrap_or(unit);
    let unit = unit.strip_suffix('s').unwrap_or(unit);
    !count.is_empty() && matches!(unit, "day" | "week" | "month" | "year")
}

#[cfg(test)]
mod tests {
    use super::{Change, Search};

    fn parse(q: &str) -> Search {
        Search::parse(q).unwrap()
    }

    #[test]
    fn plain_words_are_the_text() {
        let s = parse("  fix   the parser ");
        assert_eq!(s.text, "fix the parser");
        assert!(!s.is_qualified());
        assert_eq!(s.log_args(), Vec::<String>::new());
    }

    #[test]
    fn code_searches_what_a_commit_changed() {
        let s = parse("code:render_row");
        assert_eq!(s.change, Some(Change::Text("render_row".into())));
        assert_eq!(s.text, "");
        assert_eq!(s.log_args(), ["-Srender_row"]);
    }

    #[test]
    fn a_quoted_value_keeps_its_spaces() {
        let s = parse(r#"code:"fn main()" tidy"#);
        assert_eq!(s.change, Some(Change::Text("fn main()".into())));
        assert_eq!(s.text, "tidy");
    }

    #[test]
    fn slashes_make_the_value_a_pattern() {
        let s = parse("code:/row_(height|width)/");
        assert_eq!(s.change, Some(Change::Pattern("row_(height|width)".into())));
        assert_eq!(s.log_args(), ["-Grow_(height|width)"]);
        // A lone slash is a string that happens to be one.
        assert_eq!(parse("code:/").change, Some(Change::Text("/".into())));
        assert_eq!(parse("code://").change, Some(Change::Text("//".into())));
    }

    #[test]
    fn dates_bound_the_search() {
        let s = parse("after:2024-05-01 before:2.weeks blame");
        assert_eq!(s.after.as_deref(), Some("2024-05-01"));
        assert_eq!(s.before.as_deref(), Some("2.weeks"));
        assert_eq!(s.text, "blame");
        assert_eq!(
            s.log_args(),
            ["--since-as-filter=2024-05-01", "--until=2.weeks"]
        );
    }

    #[test]
    fn reads_the_relative_dates_people_write() {
        for ok in [
            "3.days",
            "1.week",
            "6months",
            "2.YEARS",
            "yesterday",
            "today",
        ] {
            assert!(Search::parse(&format!("after:{ok}")).is_ok(), "{ok}");
        }
    }

    #[test]
    fn refuses_a_date_git_would_quietly_misread() {
        for bad in [
            "2024-13-01",
            "2024-5-1",
            "last-tuesday",
            "soon",
            "3.fortnights",
            "week",
        ] {
            let err = Search::parse(&format!("before:{bad}")).unwrap_err();
            assert_eq!(err.code(), "refused", "{bad}");
            assert_eq!(
                err.to_string(),
                format!(
                    "cannot search by date: before:{bad} is not a date; \
                     write before:2024-05-01 or before:2.weeks"
                )
            );
        }
    }

    #[test]
    fn a_subject_prefix_is_text_not_a_qualifier() {
        // Commit subjects lead with these, and searching for one has to keep working.
        let s = parse("ui: smb/client: fix");
        assert_eq!(s.text, "ui: smb/client: fix");
        assert!(!s.is_qualified());
    }

    #[test]
    fn a_qualifier_still_being_typed_is_text() {
        let s = parse("code:");
        assert_eq!(s.text, "code:");
        assert!(!s.is_qualified());
        assert!(Search::parse("after:").is_ok());
    }

    #[test]
    fn qualifiers_are_read_whatever_their_case() {
        assert!(parse("Code:x").change.is_some());
        assert!(parse("AFTER:today").after.is_some());
    }

    #[test]
    fn a_value_that_looks_like_an_option_stays_inside_its_argument() {
        // `-S--output=x` is one argument to git, which reads it as -S with a strange string.
        assert_eq!(parse("code:--output=x").log_args(), ["-S--output=x"]);
    }
}
