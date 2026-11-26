use std::process::Command;

/// The one-line summary `coral --help` gives a command.
fn summary(of: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_coral"))
        .arg("--help")
        .output()
        .expect("run coral");
    let help = String::from_utf8_lossy(&out.stdout).into_owned();
    let lines: Vec<&str> = help.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.split_whitespace().next() == Some(of))
        .unwrap_or_else(|| panic!("{of} is not listed"));
    let mut text = lines[at].trim_start()[of.len()..].trim().to_owned();
    // A summary that wraps carries on, indented, on the lines after.
    for next in &lines[at + 1..] {
        if !next.starts_with("                 ") {
            break;
        }
        text.push(' ');
        text.push_str(next.trim());
    }
    text
}

#[test]
fn take_and_discard_each_say_what_they_do() {
    // Discard's line sat above Take when Take was added between them, so Take began "Throw
    // away worktree changes", the opposite of what it does, and Discard said nothing.
    let take = summary("take");
    assert!(
        take.starts_with("Write files into the working tree"),
        "take: {take}"
    );
    assert!(!take.contains("Throw away"), "take: {take}");
    let discard = summary("discard");
    assert!(
        discard.starts_with("Throw away worktree changes"),
        "discard: {discard}"
    );
}
