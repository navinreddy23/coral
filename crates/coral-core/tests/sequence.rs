use bstr::ByteSlice as _;

use coral_core::sequence::{Step, Todo, TodoItem};

fn item(step: Step, oid: &str, summary: &str) -> TodoItem {
    TodoItem {
        step,
        oid: oid.to_owned(),
        summary: summary.into(),
        message: None,
    }
}

/// A todo file as git writes it, including the comment block it appends.
const REAL: &[u8] = b"pick 1a2b3c4 first commit\n\
pick 5d6e7f8 second commit\n\
\n\
# Rebase abc123..def456 onto abc123 (2 commands)\n\
#\n\
# Commands:\n\
# p, pick <commit> = use commit\n";

#[test]
fn parses_a_todo_file_and_ignores_its_comment_block() {
    let todo = Todo::parse(REAL).unwrap();

    assert_eq!(todo.items.len(), 2, "the comment block is not a step");
    assert_eq!(todo.items[0].step, Step::Pick);
    assert_eq!(todo.items[0].oid, "1a2b3c4");
    assert_eq!(todo.items[0].summary, "first commit");
    assert_eq!(todo.items[1].oid, "5d6e7f8");
}

#[test]
fn accepts_both_the_long_and_short_verbs_git_writes() {
    let todo = Todo::parse(b"p a one\nr b two\ne c three\ns d four\nf e five\nd f six\n").unwrap();
    let steps: Vec<_> = todo.items.iter().map(|i| i.step).collect();

    assert_eq!(
        steps,
        vec![
            Step::Pick,
            Step::Reword,
            Step::Edit,
            Step::Squash,
            Step::Fixup,
            Step::Drop
        ]
    );
}

/// `--rebase-merges` emits label/merge/reset steps. Silently dropping them would rewrite the
/// user's history differently from what they asked for, so parsing refuses instead.
#[test]
fn refuses_steps_it_does_not_understand() {
    let err = Todo::parse(b"pick a one\nlabel onto\n").unwrap_err();
    assert_eq!(err.code(), "protocol_error");
    assert!(err.to_string().contains("label"));
}

#[test]
fn renders_back_to_a_file_git_would_accept() {
    let todo = Todo::parse(REAL).unwrap();
    let rendered = todo.render();

    assert_eq!(
        rendered,
        "pick 1a2b3c4 first commit\npick 5d6e7f8 second commit\n"
    );
    // Round-tripping must be stable.
    assert_eq!(Todo::parse(&rendered).unwrap(), todo);
}

#[test]
fn a_dropped_commit_is_written_explicitly_rather_than_omitted() {
    let mut todo = Todo::parse(b"pick a one\npick b two\n").unwrap();
    todo.items[1].step = Step::Drop;

    assert_eq!(todo.render(), "pick a one\ndrop b two\n");
}

#[test]
fn reorders_items() {
    let mut todo = Todo::parse(b"pick a one\npick b two\npick c three\n").unwrap();

    todo.reorder(2, 0).unwrap();
    let order: Vec<&str> = todo.items.iter().map(|i| i.oid.as_str()).collect();
    assert_eq!(order, vec!["c", "a", "b"]);

    assert!(todo.reorder(0, 9).is_err());
    assert!(todo.reorder(9, 0).is_err());
    assert_eq!(todo.reorder(0, 9).unwrap_err().code(), "refused");
}

/// Squashing the first commit has nothing to fold into, and git fails partway through the
/// rebase rather than up front. Catching it before starting is the whole point.
#[test]
fn rejects_a_first_step_that_has_nothing_to_squash_into() {
    let mut todo = Todo::parse(b"pick a one\npick b two\n").unwrap();
    assert!(todo.first_step_is_valid());

    todo.items[0].step = Step::Squash;
    assert!(!todo.first_step_is_valid());

    todo.items[0].step = Step::Fixup;
    assert!(!todo.first_step_is_valid());

    todo.items[0].step = Step::Drop;
    assert!(
        todo.first_step_is_valid(),
        "dropping the first commit is legal"
    );
}

#[test]
fn an_empty_or_comment_only_file_parses_to_nothing() {
    assert!(Todo::parse(b"").unwrap().items.is_empty());
    assert!(
        Todo::parse(b"# nothing to do\n\n")
            .unwrap()
            .items
            .is_empty()
    );
    assert!(Todo::default().render().is_empty());
}

#[test]
fn a_summary_containing_spaces_survives() {
    let todo = Todo::parse(b"pick abc a summary with several words\n").unwrap();
    assert_eq!(todo.items[0].summary, "a summary with several words");
    assert_eq!(todo.render(), "pick abc a summary with several words\n");
}

#[test]
fn a_step_with_no_summary_round_trips() {
    let todo = Todo::parse(b"pick abc123\n").unwrap();
    assert_eq!(todo.items[0].oid, "abc123");
    assert!(todo.items[0].summary.is_empty());
    assert_eq!(todo.render(), "pick abc123\n");
}

#[test]
fn renders_every_verb_git_understands() {
    let all = Todo {
        items: vec![
            item(Step::Pick, "a", "x"),
            item(Step::Reword, "b", "x"),
            item(Step::Edit, "c", "x"),
            item(Step::Squash, "d", "x"),
            item(Step::Fixup, "e", "x"),
            item(Step::Drop, "f", "x"),
        ],
    };
    let rendered = all.render();
    for verb in ["pick", "reword", "edit", "squash", "fixup", "drop"] {
        assert!(
            rendered.contains_str(verb),
            "{verb} missing from the rendered todo"
        );
    }
    assert_eq!(Todo::parse(&rendered).unwrap(), all);
}
