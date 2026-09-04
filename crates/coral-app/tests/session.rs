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

/// Three loose tabs, with the first two grouped as "work".
fn grouped() -> (Session, u32) {
    let mut s = Session::default();
    for path in ["/a", "/b", "/c"] {
        s.open(std::path::PathBuf::from(path));
    }
    let ids: Vec<u32> = s.tabs.iter().map(|t| t.id).collect();
    let group = s.group("work".to_owned(), &[ids[0], ids[1]]);
    (s, group)
}

fn order(s: &Session) -> Vec<(String, Option<u32>)> {
    s.tabs
        .iter()
        .map(|t| (t.path.to_string_lossy().into_owned(), t.group))
        .collect()
}

#[test]
fn a_tab_can_be_dragged_into_an_existing_group() {
    // There was no way to do this at all: a group could be created and a tab removed from one,
    // but nothing put a tab into a group that already existed.
    let (mut s, group) = grouped();
    let c = s.tabs.iter().find(|t| t.path.ends_with("c")).unwrap().id;

    s.move_tab(c, Some(group), None);

    assert_eq!(
        order(&s),
        vec![
            ("/a".to_owned(), Some(group)),
            ("/b".to_owned(), Some(group)),
            ("/c".to_owned(), Some(group)),
        ]
    );
}

#[test]
fn a_tab_dropped_on_a_group_lands_beside_its_new_neighbours() {
    // Not at the end of the whole bar, which is where it would go if the group were ignored.
    let mut s = Session::default();
    for path in ["/a", "/b", "/c", "/d"] {
        s.open(std::path::PathBuf::from(path));
    }
    let ids: Vec<u32> = s.tabs.iter().map(|t| t.id).collect();
    let group = s.group("work".to_owned(), &[ids[0], ids[1]]);

    s.move_tab(ids[3], Some(group), None);

    let paths: Vec<String> = s
        .tabs
        .iter()
        .map(|t| t.path.to_string_lossy().into_owned())
        .collect();
    assert_eq!(paths, ["/a", "/b", "/d", "/c"]);
}

#[test]
fn a_tab_dragged_out_of_a_group_becomes_loose() {
    let (mut s, group) = grouped();
    let b = s.tabs.iter().find(|t| t.path.ends_with("b")).unwrap().id;

    s.move_tab(b, None, None);

    let moved = s.tabs.iter().find(|t| t.id == b).unwrap();
    assert_eq!(moved.group, None);
    // The group survives, because one tab is still in it.
    assert!(s.groups.iter().any(|g| g.id == group));
}

#[test]
fn emptying_a_group_by_dragging_removes_it() {
    let (mut s, group) = grouped();
    let ids: Vec<u32> = s
        .tabs
        .iter()
        .filter(|t| t.group == Some(group))
        .map(|t| t.id)
        .collect();
    for id in ids {
        s.move_tab(id, None, None);
    }
    assert!(
        s.groups.is_empty(),
        "a band with nothing in it is not drawn"
    );
}

#[test]
fn a_tab_can_be_dropped_in_front_of_another() {
    // Reordering, which is the other half of what dragging a tab is expected to do.
    let (mut s, _) = grouped();
    let ids: Vec<u32> = s.tabs.iter().map(|t| t.id).collect();
    let (a, c) = (ids[0], ids[2]);

    s.move_tab(c, None, Some(a));

    let paths: Vec<String> = s
        .tabs
        .iter()
        .map(|t| t.path.to_string_lossy().into_owned())
        .collect();
    // /c is loose and /a is grouped, so contiguity puts the loose one first.
    assert_eq!(paths[0], "/c");
}

#[test]
fn dropping_a_tab_on_itself_changes_nothing() {
    let (mut s, group) = grouped();
    let a = s.tabs.iter().find(|t| t.path.ends_with("a")).unwrap().id;
    let before = order(&s);

    s.move_tab(a, Some(group), Some(a));

    assert_eq!(order(&s), before, "and above all it is still there");
}

#[test]
fn a_group_that_no_longer_exists_leaves_the_tab_loose() {
    // Rather than stranded in a band nothing draws.
    let (mut s, group) = grouped();
    let c = s.tabs.iter().find(|t| t.path.ends_with("c")).unwrap().id;
    s.groups.clear();

    s.move_tab(c, Some(group), None);

    assert_eq!(s.tabs.iter().find(|t| t.id == c).unwrap().group, None);
}

#[test]
fn moving_a_tab_that_is_not_there_does_nothing() {
    let (mut s, group) = grouped();
    let before = order(&s);
    s.move_tab(9999, Some(group), None);
    assert_eq!(order(&s), before);
}
