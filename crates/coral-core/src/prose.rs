//! Which words of a commit message are prose, and so worth checking for spelling.
//!
//! A message is English interleaved with things that are not: identifiers, paths, hashes, URLs,
//! code in backticks, trailers. Anything that might be one of those is left out. An underline
//! under a function name is noise the writer learns to ignore, and then the real ones go
//! unread too.

/// A word to check, and the byte offset it starts at in the text it was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Word<'a> {
    pub(crate) text: &'a str,
    pub(crate) at: usize,
}

// Any of these inside a chunk makes it something other than a word: a path, a file name, a
// call, a URL, an email, an issue number, a mention, an expression.
const SYMBOLS: &[char] = &[
    '/', '\\', '_', '.', ':', '=', '<', '>', '(', ')', '[', ']', '{', '}', '|', '~', '$', '%', '^',
    '&', '*', '+', '#', '@', '`',
];
const LEADING: &[char] = &['(', '[', '{', '<', '"', '\'', '\u{2018}', '\u{201c}', '*'];
const TRAILING: &[char] = &[
    '.', ',', ';', ':', '!', '?', ')', ']', '}', '>', '"', '\'', '\u{2019}', '\u{201d}', '*',
];
const SENTENCE: &[char] = &['.', ',', ';', ':', '!', '?', ')', ']', '}'];
const DASHES: &[char] = &['-', '\u{2013}', '\u{2014}'];
const APOSTROPHES: &[char] = &['\'', '\u{2019}'];
const MARKERS: &[&str] = &["fixup! ", "squash! ", "amend! "];

/// The prose words of a message's first line.
pub(crate) fn summary(text: &str) -> Vec<Word<'_>> {
    let mut words = Vec::new();
    line(text, past_prefix(text), text.len(), &mut words);
    words
}

/// The prose words of everything after the first line.
pub(crate) fn body(text: &str) -> Vec<Word<'_>> {
    let mut words = Vec::new();
    let end = trailers_start(text);
    let mut fenced = false;
    for (at, content) in lines(text) {
        if at >= end {
            break;
        }
        let trimmed = content.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        // Indented lines are pasted output or code, as in Markdown.
        if fenced || content.starts_with("    ") || content.starts_with('\t') {
            continue;
        }
        line(text, at, at + content.len(), &mut words);
    }
    words
}

/// Where the summary's own words start: after any `fixup! ` markers git's autosquash writes,
/// and after an area prefix such as `core:` or `feat(ui)!:`, which names a part of the
/// project rather than saying anything in English.
fn past_prefix(text: &str) -> usize {
    let mut at = 0;
    while let Some(marker) = MARKERS.iter().find(|m| text[at..].starts_with(**m)) {
        at += marker.len();
    }
    let rest = &text[at..];
    let first = rest.split(char::is_whitespace).next().unwrap_or_default();
    if first.len() > 1 && first.ends_with(':') && rest[first.len()..].starts_with(' ') {
        at += first.len();
    }
    at
}

/// Each line with the byte offset it starts at, line endings left off.
fn lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.split_inclusive('\n').scan(0, |at, raw| {
        let start = *at;
        *at += raw.len();
        Some((start, raw.trim_end_matches(['\n', '\r'])))
    })
}

/// Where the trailer block starts, or the end of the text when there is none. Like git, the
/// block is the last paragraph, and only when every line in it is a `Token: value` trailer.
/// See <https://git-scm.com/docs/git-interpret-trailers>.
fn trailers_start(text: &str) -> usize {
    let mut paragraph = None;
    let mut all_trailers = false;
    for (at, content) in lines(text) {
        if content.trim().is_empty() {
            paragraph = None;
            continue;
        }
        if paragraph.is_none() {
            paragraph = Some(at);
            all_trailers = true;
        }
        all_trailers &= is_trailer(content) || content.starts_with([' ', '\t']);
    }
    match paragraph {
        Some(at) if all_trailers => at,
        _ => text.len(),
    }
}

fn is_trailer(content: &str) -> bool {
    let Some((token, value)) = content.split_once(':') else {
        return false;
    };
    !token.is_empty()
        && token.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && value.starts_with(' ')
}

/// The prose words of `text[start..end]`, a single line, leaving out code spans.
fn line<'a>(text: &'a str, start: usize, end: usize, words: &mut Vec<Word<'a>>) {
    let mut from = start;
    for (open, close) in code_spans(&text[start..end]) {
        chunks(text, from, start + open, words);
        from = start + close;
    }
    chunks(text, from, end, words);
}

/// The byte ranges of the inline code spans in a line, backticks included. A run of backticks
/// closes only on a run of the same length, as Markdown has it; one that never closes is left
/// as it is, and its chunk is not a word anyway.
fn code_spans(content: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut at = 0;
    while let Some(open) = content[at..].find('`').map(|i| at + i) {
        let fence = run_of_backticks(&content[open..]);
        let mut search = open + fence;
        let close = loop {
            let Some(next) = content[search..].find('`').map(|i| search + i) else {
                break None;
            };
            let run = run_of_backticks(&content[next..]);
            if run == fence {
                break Some(next + run);
            }
            search = next + run;
        };
        match close {
            Some(close) => {
                spans.push((open, close));
                at = close;
            }
            None => at = open + fence,
        }
    }
    spans
}

fn run_of_backticks(text: &str) -> usize {
    text.bytes().take_while(|b| *b == b'`').count()
}

/// The words of `text[start..end]`, split on whitespace into chunks first.
fn chunks<'a>(text: &'a str, start: usize, end: usize, words: &mut Vec<Word<'a>>) {
    let mut chunk_start = None;
    for (i, c) in text[start..end].char_indices() {
        match (c.is_whitespace(), chunk_start) {
            (true, Some(from)) => {
                chunk(text, start + from, start + i, words);
                chunk_start = None;
            }
            (false, None) => chunk_start = Some(i),
            _ => {}
        }
    }
    if let Some(from) = chunk_start {
        chunk(text, start + from, end, words);
    }
}

/// The words of one whitespace-free chunk, or none when the chunk is not prose.
fn chunk<'a>(text: &'a str, start: usize, end: usize, words: &mut Vec<Word<'a>>) {
    let raw = &text[start..end];
    if is_quoted_name(raw.trim_end_matches(SENTENCE)) {
        return;
    }
    let core = raw.trim_start_matches(LEADING);
    let at = start + (raw.len() - core.len());
    let core = core.trim_end_matches(TRAILING);
    if core.is_empty() || core.contains(SYMBOLS) || core.contains(|c: char| c.is_ascii_digit()) {
        return;
    }
    let mut from = 0;
    for part in core.split(DASHES) {
        word(text, at + from, part, words);
        from += part.len() + part_separator(&core[from + part.len()..]);
    }
}

fn part_separator(rest: &str) -> usize {
    rest.chars().next().map_or(0, char::len_utf8)
}

/// A branch or file named in quotes, `'feature-x'`, which reads as a name and not a word.
fn is_quoted_name(chunk: &str) -> bool {
    let pairs = [
        ('\'', '\''),
        ('"', '"'),
        ('\u{2018}', '\u{2019}'),
        ('\u{201c}', '\u{201d}'),
    ];
    pairs.iter().any(|(open, close)| {
        chunk.chars().count() > 2 && chunk.starts_with(*open) && chunk.ends_with(*close)
    })
}

/// Adds `part`, at byte `at` of `text`, when it reads as a word of English.
fn word<'a>(text: &'a str, at: usize, part: &str, words: &mut Vec<Word<'a>>) {
    let trimmed = part.trim_start_matches(APOSTROPHES);
    let at = at + (part.len() - trimmed.len());
    let trimmed = trimmed.trim_end_matches(APOSTROPHES);
    if is_checkable(trimmed) {
        words.push(Word {
            text: &text[at..at + trimmed.len()],
            at,
        });
    }
}

/// Letters of the Latin alphabet, two or more, not an acronym and not mixed case. An English
/// dictionary has nothing to say about another script, and `GitHub`, `iPhone` or `HTTP` are
/// names rather than words it could know.
fn is_checkable(part: &str) -> bool {
    let mut chars = part.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let rest: Vec<char> = chars.collect();
    let letters = rest.iter().filter(|c| !APOSTROPHES.contains(c)).count() + 1;
    letters >= 2
        && is_latin(first)
        && rest.iter().all(|c| APOSTROPHES.contains(c) || is_latin(*c))
        && !rest.iter().any(|c| c.is_uppercase())
}

fn is_latin(c: char) -> bool {
    c.is_ascii_alphabetic()
        || (c.is_alphabetic()
            && matches!(u32::from(c), 0x00C0..=0x024F | 0x1E00..=0x1EFF)
            && c != '\u{00D7}'
            && c != '\u{00F7}')
}

#[cfg(test)]
mod tests {
    use super::{Word, body, summary};

    fn texts(words: &[Word<'_>]) -> Vec<String> {
        words.iter().map(|w| w.text.to_owned()).collect()
    }

    fn summary_words(text: &str) -> Vec<String> {
        texts(&summary(text))
    }

    fn body_words(text: &str) -> Vec<String> {
        texts(&body(text))
    }

    #[test]
    fn reads_the_plain_words_of_a_summary() {
        assert_eq!(summary_words("Fix the watcher"), ["Fix", "the", "watcher"]);
        assert_eq!(summary_words("  two  spaces "), ["two", "spaces"]);
        assert_eq!(summary_words(""), Vec::<String>::new());
    }

    #[test]
    fn says_where_each_word_starts() {
        let text = "core: teh  watcher";
        for word in summary(text) {
            assert_eq!(&text[word.at..word.at + word.text.len()], word.text);
        }
        let at: Vec<usize> = summary(text).iter().map(|w| w.at).collect();
        assert_eq!(at, [6, 11]);
    }

    #[test]
    fn leaves_out_the_area_prefix_of_a_summary() {
        let cases: &[(&str, &[&str])] = &[
            ("core: check the spelling", &["check", "the", "spelling"]),
            ("feat(ui)!: underline words", &["underline", "words"]),
            ("ui/staging: underline words", &["underline", "words"]),
            ("fixup! core: check it", &["check", "it"]),
            ("fixup! fixup! squash! word", &["word"]),
            ("amend! Fix the watcher", &["Fix", "the", "watcher"]),
            // Only a prefix that ends its chunk and is followed by a space.
            ("core:check", &[]),
            ("Fix: typo in the readme", &["typo", "in", "the", "readme"]),
        ];
        for (text, expected) in cases {
            assert_eq!(summary_words(text), *expected, "{text}");
        }
    }

    #[test]
    fn leaves_out_anything_that_reads_as_code_or_a_name() {
        let cases: &[(&str, &[&str])] = &[
            ("Read ops.rs and Cargo.toml", &["Read", "and"]),
            ("Move src/app to lib", &["Move", "to", "lib"]),
            ("Call walk_tips() once", &["Call", "once"]),
            ("Return Vec<String> now", &["Return", "now"]),
            ("Use RowStore and GitHub", &["Use", "and"]),
            ("Support iPhone users", &["Support", "users"]),
            ("Speak HTTP and JSON", &["Speak", "and"]),
            ("Close #123 for @ada", &["Close", "for"]),
            ("See https://example.com/x now", &["See", "now"]),
            ("Mail ada@example.invalid today", &["Mail", "today"]),
            ("Revert 1a2b3c4d5e and v2", &["Revert", "and"]),
            ("Add :sparkles: emoji", &["Add", "emoji"]),
            ("Set a=b or x+y", &["Set", "or"]),
            (
                "Merge branch 'feature-x' into main",
                &["Merge", "branch", "into", "main"],
            ),
            (
                "Merge branch \u{2018}topic\u{2019} into main",
                &["Merge", "branch", "into", "main"],
            ),
            ("e.g. this, i.e. that", &["this", "that"]),
        ];
        for (text, expected) in cases {
            assert_eq!(summary_words(text), *expected, "{text}");
        }
    }

    #[test]
    fn leaves_out_inline_code() {
        let cases: &[(&str, &[&str])] = &[
            ("Call `frobnicate` twice", &["Call", "twice"]),
            ("Call ``a ` b`` twice", &["Call", "twice"]),
            ("Call `two words` twice", &["Call", "twice"]),
            ("Run `a` and `b` then", &["Run", "and", "then"]),
            // An unclosed run is left as text, and its chunk is not a word.
            ("A stray ` backtick", &["stray", "backtick"]),
            ("A `stray backtick", &["backtick"]),
        ];
        for (text, expected) in cases {
            assert_eq!(summary_words(text), *expected, "{text}");
        }
    }

    #[test]
    fn strips_punctuation_and_quotes_around_a_word() {
        let cases: &[(&str, &[&str])] = &[
            (
                "Fix it (properly), then stop.",
                &["Fix", "it", "properly", "then", "stop"],
            ),
            (
                "Say \"hello world\" once",
                &["Say", "hello", "world", "once"],
            ),
            ("Really? Yes! Done;", &["Really", "Yes", "Done"]),
            ("*very* careful", &["very", "careful"]),
            ("[draft] {note} <later>", &["draft", "note", "later"]),
        ];
        for (text, expected) in cases {
            assert_eq!(summary_words(text), *expected, "{text}");
        }
    }

    #[test]
    fn splits_hyphens_and_keeps_apostrophes() {
        let cases: &[(&str, &[&str])] = &[
            ("a well-known fix", &["well", "known", "fix"]),
            ("off-by-one", &["off", "by", "one"]),
            (
                "pre\u{2013}post and this\u{2014}that",
                &["pre", "post", "and", "this", "that"],
            ),
            ("don't stop", &["don't", "stop"]),
            ("it\u{2019}s fine", &["it\u{2019}s", "fine"]),
            ("the users' files", &["the", "users", "files"]),
            ("Coral's window", &["Coral's", "window"]),
            ("PR's and PRs", &["and"]),
        ];
        for (text, expected) in cases {
            assert_eq!(summary_words(text), *expected, "{text}");
        }
    }

    #[test]
    fn keeps_latin_letters_and_leaves_other_scripts_alone() {
        assert_eq!(
            summary_words("a naïve café façade"),
            ["naïve", "café", "façade"]
        );
        assert_eq!(summary_words("Ünïcode Straße"), ["Ünïcode", "Straße"]);
        assert_eq!(summary_words("修复 the λόγος bug"), ["the", "bug"]);
        assert_eq!(summary_words("2×3 x÷y"), Vec::<String>::new());
    }

    #[test]
    fn leaves_out_fenced_and_indented_code_in_the_body() {
        let text = "Explain the change.\n\n```rust\nlet frobnicate = 1;\n```\n\n    $ coral spell\n\tteh output\nBack in prose.\n~~~\nfenced tildes\n~~~\nDone.";
        assert_eq!(
            body_words(text),
            ["Explain", "the", "change", "Back", "in", "prose", "Done"]
        );
    }

    #[test]
    fn leaves_out_a_closing_trailer_paragraph() {
        let text = "Fix the watcher.\n\nSigned-off-by: Ada Lovelace <ada@example.invalid>\nCo-authored-by: Grace Hopper <grace@example.invalid>\n";
        assert_eq!(body_words(text), ["Fix", "the", "watcher"]);

        // A trailer that continues on an indented line is still a trailer.
        let text = "Body text.\n\nReviewed-by: Somebody\n  Withalongname\n";
        assert_eq!(body_words(text), ["Body", "text"]);

        // Only the last paragraph, and only when all of it is trailers.
        let text = "Note: this matters\n\nand this too";
        assert_eq!(
            body_words(text),
            ["Note", "this", "matters", "and", "this", "too"]
        );
        let text = "Prose first.\n\nAcked-by: Someone\nbut then prose";
        assert_eq!(
            body_words(text),
            [
                "Prose", "first", "Acked", "by", "Someone", "but", "then", "prose"
            ]
        );
    }

    #[test]
    fn reads_a_body_across_lines_with_their_offsets() {
        let text = "First line\r\nsecond line\n\nthird";
        let words = body(text);
        assert_eq!(texts(&words), ["First", "line", "second", "line", "third"]);
        for word in words {
            assert_eq!(&text[word.at..word.at + word.text.len()], word.text);
        }
    }

    #[test]
    fn finds_nothing_in_an_empty_or_symbol_only_message() {
        assert_eq!(body(""), []);
        assert_eq!(body("\n\n\n"), []);
        assert_eq!(body("# 123\n--- ***"), []);
        assert_eq!(summary("-"), []);
    }
}
