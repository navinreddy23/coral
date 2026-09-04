//! Every action, in the exact JSON the window sends.
//!
//! `ui/src/ipc/commands.ts` writes the `Action` union by hand, because ts-rs generates from
//! `coral-core` and this type lives in the command layer. Nothing but this file connects the
//! two, and without it a field named in one and not the other is found by a user pressing a
//! button: Push spent its whole life answering "missing field `set_upstream`" to a window that
//! had faithfully sent `setUpstream`.
//!
//! The strings below are the payloads as they go over the wire. Copy them from the call site,
//! not from the Rust type, or the test proves only that Rust agrees with itself.

use coral_app_lib::actions::Action;

#[track_caller]
fn accepts(json: &str) -> Action {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}\n  {e}"))
}

#[test]
fn every_action_the_window_can_send_is_understood() {
    for json in [
        r#"{"kind":"fetch","remote":null}"#,
        r#"{"kind":"pull","remote":null,"mode":"ffOnly"}"#,
        r#"{"kind":"pull","remote":"origin","mode":"rebase"}"#,
        r#"{"kind":"push","remote":null,"setUpstream":true}"#,
        r#"{"kind":"checkout","rev":"main"}"#,
        r#"{"kind":"branchCreate","name":"topic","at":null,"checkout":true}"#,
        r#"{"kind":"branchDelete","name":"topic","force":false}"#,
        r#"{"kind":"merge","rev":"topic"}"#,
        r#"{"kind":"rebase","onto":"main"}"#,
        r#"{"kind":"cherryPick","revs":["abc"],"commit":true}"#,
        r#"{"kind":"cherryPick","revs":["abc"],"commit":false}"#,
        r#"{"kind":"revert","revs":["abc"]}"#,
        r#"{"kind":"stashPush","message":null}"#,
        r#"{"kind":"stashApply","index":0,"pop":true}"#,
        r#"{"kind":"stashDrop","index":0}"#,
        r#"{"kind":"tagCreate","name":"v1","at":null,"message":null}"#,
        r#"{"kind":"tagDelete","name":"v1"}"#,
        r#"{"kind":"reset","rev":"abc","mode":"hard"}"#,
        r#"{"kind":"rewrite","rev":"abc","how":"moveNewer","message":null}"#,
        r#"{"kind":"worktreeAdd","path":"/tmp/wt","rev":"abc","branch":null}"#,
        r#"{"kind":"submoduleInit","path":null,"recursive":true,"remote":false}"#,
        r#"{"kind":"submoduleSetUrl","path":"lib/x","url":"https://example.invalid/x.git"}"#,
        r#"{"kind":"submoduleRemove","path":"lib/x","force":false}"#,
        r#"{"kind":"patch","rev":"abc","directory":"/tmp"}"#,
        r#"{"kind":"undo"}"#,
        r#"{"kind":"redo"}"#,
    ] {
        accepts(json);
    }
}

#[test]
fn the_field_that_was_wrong_is_named_the_way_the_window_names_it() {
    // Stated on its own, so that a change to the naming convention fails here with the reason
    // rather than somewhere in the list above with a serde message.
    let pushed = accepts(r#"{"kind":"push","remote":"origin","setUpstream":true}"#);
    assert!(matches!(
        pushed,
        Action::Push {
            set_upstream: true,
            ..
        }
    ));
    assert!(
        serde_json::from_str::<Action>(r#"{"kind":"push","remote":null,"set_upstream":true}"#)
            .is_err(),
        "snake_case is not what the window sends, and accepting both hides the next mismatch"
    );
}
