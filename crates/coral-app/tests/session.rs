//! Tabs and groups. The property that matters is that a session survives a restart intact,
//! including a repository that has gone missing in the meantime.

use coral_app_lib::session::{GroupColour, Session};
use coral_core::testutil::TestRepo;

fn saved(session: &Session, dir: &tempfile::TempDir) -> Session {
    let path = dir.path().join("session.json");
    session.save(&path).unwrap();
    Session::load(&path)
}

#[test]
fn opening_the_same_repository_twice_focuses_the_existing_tab() {
    let mut s = Session::default();
    let first = s.open("/repos/a".into());
    let second = s.open("/repos/b".into());
    let again = s.open("/repos/a".into());

    assert_eq!(again, first, "the same path is the same tab");
    assert_eq!(s.tabs.len(), 2);
    assert_eq!(s.active, Some(first));
    assert_ne!(first, second);
}

#[test]
fn closing_a_tab_focuses_a_neighbour_rather_than_nothing() {
    let mut s = Session::default();
    let a = s.open("/repos/a".into());
    let b = s.open("/repos/b".into());
    let c = s.open("/repos/c".into());

    s.activate(b);
    s.close(b);
    assert_eq!(
        s.active,
        Some(c),
        "focus moves to the tab that took its place"
    );

    s.close(c);
    assert_eq!(s.active, Some(a));
    s.close(a);
    assert_eq!(s.active, None, "nothing left to focus");
}

#[test]
fn a_group_takes_a_colour_and_holds_its_tabs() {
    let mut s = Session::default();
    let a = s.open("/repos/a".into());
    let b = s.open("/repos/b".into());
    s.open("/repos/c".into());

    let g = s.group("kernel".into(), &[a, b]);
    let group = s.groups.iter().find(|x| x.id == g).unwrap();

    assert_eq!(group.name, "kernel");
    assert_eq!(group.colour, GroupColour::Lane1);
    assert!(!group.collapsed);
    assert_eq!(s.tabs.iter().filter(|t| t.group == Some(g)).count(), 2);
}

/// A group is drawn as one band around a run of tabs, so its members have to stay adjacent.
#[test]
fn grouped_tabs_are_kept_contiguous() {
    let mut s = Session::default();
    let a = s.open("/repos/a".into());
    s.open("/repos/middle".into());
    let c = s.open("/repos/c".into());

    let g = s.group("ends".into(), &[a, c]);
    let groups: Vec<Option<u32>> = s.tabs.iter().map(|t| t.group).collect();

    assert_eq!(
        groups,
        vec![Some(g), Some(g), None],
        "the odd one out moved aside"
    );
}

#[test]
fn consecutive_groups_take_different_colours() {
    let mut s = Session::default();
    let a = s.open("/a".into());
    let b = s.open("/b".into());
    let g1 = s.group("one".into(), &[a]);
    let g2 = s.group("two".into(), &[b]);

    let colour = |id: u32| s.groups.iter().find(|g| g.id == id).unwrap().colour;
    assert_ne!(colour(g1), colour(g2));
}

#[test]
fn a_group_disappears_once_its_last_tab_leaves() {
    let mut s = Session::default();
    let a = s.open("/a".into());
    let g = s.group("solo".into(), &[a]);
    assert_eq!(s.groups.len(), 1);

    s.ungroup(a);
    assert!(s.groups.is_empty(), "an empty group is not worth keeping");
    assert_eq!(s.tabs.len(), 1, "but the tab stays open");
    let _ = g;
}

#[test]
fn activating_a_tab_expands_the_group_it_is_hidden_in() {
    let mut s = Session::default();
    let a = s.open("/a".into());
    let g = s.group("collapsed".into(), &[a]);
    s.set_collapsed(g, true);

    s.activate(a);
    assert!(!s.groups.iter().find(|x| x.id == g).unwrap().collapsed);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_session_survives_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");

    let mut s = Session::default();
    let tab = s.open(repo.path().to_path_buf());
    let g = s.group("work".into(), &[tab]);
    s.set_collapsed(g, true);

    let restored = saved(&s, &dir);
    assert_eq!(restored.tabs.len(), 1);
    assert_eq!(restored.active, Some(tab));
    assert_eq!(restored.groups.len(), 1);
    assert_eq!(restored.groups[0].name, "work");
    assert!(
        restored.groups[0].collapsed,
        "a collapsed group comes back collapsed"
    );
    assert!(!restored.tabs[0].missing);
}

/// A disconnected drive must not silently lose someone's workspace.
#[test]
fn a_repository_that_has_gone_misses_rather_than_vanishing() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Session::default();
    s.open("/definitely/not/here".into());

    let restored = saved(&s, &dir);
    assert_eq!(restored.tabs.len(), 1, "the tab is kept");
    assert!(restored.tabs[0].missing, "and marked");
}

/// A corrupt session must never stop the application from opening.
#[test]
fn a_corrupt_session_loads_as_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.json");
    std::fs::write(&path, b"{ this is not json").unwrap();

    let s = Session::load(&path);
    assert!(s.tabs.is_empty());
    assert_eq!(s.active, None);
}

/// A hand-edited file can name a group that is not there, or focus a tab that is not open.
#[test]
fn a_session_with_dangling_references_is_repaired() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.json");
    std::fs::write(
        &path,
        br#"{"tabs":[{"id":7,"path":"/a","group":99}],"groups":[],"active":42,"nextId":0}"#,
    )
    .unwrap();

    let s = Session::load(&path);
    assert_eq!(
        s.tabs[0].group, None,
        "a group that does not exist is dropped"
    );
    assert_eq!(
        s.active,
        Some(7),
        "focus falls back to a tab that is actually open"
    );
}

/// Ids must never be reused, or a stale reference silently addresses a different repository.
#[test]
fn ids_are_not_reused_after_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Session::default();
    let a = s.open("/a".into());
    s.close(a);

    let mut restored = saved(&s, &dir);
    let b = restored.open("/b".into());
    assert_ne!(b, a, "the new tab must not inherit the closed one's id");
}
