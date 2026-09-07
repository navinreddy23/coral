//! Profiles: a workspace and an identity, and what survives a restart.
//!
//! The property that matters is that switching is lossless in both directions. A switch that
//! forgets the tabs it was asked to put away is worse than not having profiles at all, so
//! most of what follows is about the outgoing side rather than the incoming one.

use coral_app_lib::profile::{Profiles, slug};
use coral_app_lib::recent::Recents;
use coral_app_lib::session::{GroupColour, Session};
use coral_app_lib::tabs::Tabs;
use coral_core::identity::Identity;

/// A fresh config directory, as a first launch finds it.
fn config() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn a_first_launch_has_one_profile_and_is_in_it() {
    let dir = config();
    let profiles = Profiles::load(dir.path().to_path_buf());
    let reg = profiles.read();

    assert_eq!(reg.profiles.len(), 1);
    assert_eq!(reg.current, reg.profiles[0].id);
    assert_eq!(reg.profiles[0].name, "Personal");
}

#[test]
fn the_workspace_that_was_there_before_profiles_becomes_the_first_one() {
    // Somebody upgrading has eight tabs open. Losing them to a feature that exists to keep
    // tabs would be the worst possible introduction to it.
    let dir = config();
    let before = Session::default();
    before.save(&dir.path().join("session.json")).unwrap();
    std::fs::write(
        dir.path().join("recent.json"),
        br#"[{"path":"/srv/thing","name":"thing","opened":1}]"#,
    )
    .unwrap();

    let profiles = Profiles::load(dir.path().to_path_buf());
    let first = profiles.read().profiles[0].id.clone();

    assert!(
        profiles.session_path(&first).exists(),
        "the session moved into the profile"
    );
    assert!(profiles.recent_path(&first).exists());
    assert!(
        !dir.path().join("session.json").exists(),
        "and is not left behind to be read by a later launch as well"
    );
    assert_eq!(Recents::load(profiles.recent_path(&first)).read().len(), 1);
}

#[test]
fn a_registry_survives_a_restart() {
    let dir = config();
    let profiles = Profiles::load(dir.path().to_path_buf());
    let work = profiles.create("Work", GroupColour::Lane3);
    profiles.set_identity(
        &work,
        Identity {
            name: Some("Work Me".to_owned()),
            email: Some("me@work.example".to_owned()),
        },
    );
    profiles.switch(&work);

    let again = Profiles::load(dir.path().to_path_buf()).read();
    assert_eq!(again.current, work);
    let found = again.profiles.iter().find(|p| p.id == work).unwrap();
    assert_eq!(found.name, "Work");
    assert_eq!(found.colour, GroupColour::Lane3);
    assert_eq!(
        found.settings.user.email.as_deref(),
        Some("me@work.example")
    );
}

#[test]
fn a_registry_that_cannot_be_read_still_opens_the_window() {
    // A hand-edited or truncated file must leave somebody with a working application, and the
    // only safe answer is the profile everybody starts with.
    let dir = config();
    std::fs::write(dir.path().join("profiles.json"), b"{ not json").unwrap();

    let reg = Profiles::load(dir.path().to_path_buf()).read();
    assert_eq!(reg.profiles.len(), 1);
    assert_eq!(reg.current, reg.profiles[0].id);
}

#[test]
fn a_registry_naming_a_profile_that_is_gone_falls_back_rather_than_showing_nothing() {
    let dir = config();
    std::fs::write(
        dir.path().join("profiles.json"),
        br#"{"profiles":[{"id":"work","name":"Work","colour":"lane1"}],"current":"deleted"}"#,
    )
    .unwrap();

    let reg = Profiles::load(dir.path().to_path_buf()).read();
    assert_eq!(reg.current, "work");
}

#[test]
fn switching_puts_one_workspace_away_and_takes_the_other_out() {
    let dir = config();
    let profiles = Profiles::load(dir.path().to_path_buf());
    let personal = profiles.read().current;
    let work = profiles.create("Work", GroupColour::Lane2);

    let tabs = Tabs::load(profiles.session_path(&personal));
    tabs.opened("/srv/personal-one");
    tabs.opened("/srv/personal-two");
    assert_eq!(tabs.read().tabs.len(), 2);

    profiles.switch(&work);
    let empty = tabs.switch_to(profiles.session_path(&work));
    assert!(empty.tabs.is_empty(), "a new profile opens on nothing");
    tabs.opened("/srv/work-one");

    profiles.switch(&personal);
    let back = tabs.switch_to(profiles.session_path(&personal));
    assert_eq!(back.tabs.len(), 2, "both are still there");
    assert_eq!(back.tabs[0].path.display().to_string(), "/srv/personal-one");

    // And the outgoing side was written, not just held: a crash after a switch loses nothing.
    let saved = Session::load(&profiles.session_path(&work));
    assert_eq!(saved.tabs.len(), 1);
}

#[test]
fn recents_switch_with_the_tabs() {
    let dir = config();
    let profiles = Profiles::load(dir.path().to_path_buf());
    let personal = profiles.read().current;
    let work = profiles.create("Work", GroupColour::Lane2);

    let recents = Recents::load(profiles.recent_path(&personal));
    recents.opened("/srv/personal-one");

    recents.switch_to(profiles.recent_path(&work));
    assert!(recents.read().is_empty(), "work has its own history");

    recents.switch_to(profiles.recent_path(&personal));
    assert_eq!(recents.read().len(), 1);
}

#[test]
fn the_last_profile_cannot_be_deleted() {
    // There is no state in which the application has no profile, because there would then be
    // nowhere to put the tabs somebody opens next.
    let dir = config();
    let profiles = Profiles::load(dir.path().to_path_buf());
    let only = profiles.read().current;

    assert!(!profiles.delete(&only));
    assert_eq!(profiles.read().profiles.len(), 1);
}

#[test]
fn deleting_the_current_profile_lands_in_another_one() {
    let dir = config();
    let profiles = Profiles::load(dir.path().to_path_buf());
    let personal = profiles.read().current;
    let work = profiles.create("Work", GroupColour::Lane2);
    profiles.switch(&work);

    // Something of its own on disk, so the removal can be seen to have happened.
    Tabs::load(profiles.session_path(&work)).opened("/srv/work-one");
    let gone = profiles.session_path(&work);
    assert!(gone.exists());

    assert!(profiles.delete(&work));
    let reg = profiles.read();
    assert_eq!(reg.current, personal);
    assert_eq!(reg.profiles.len(), 1);
    assert!(!gone.exists(), "its workspace goes with it");
}

#[test]
fn an_id_is_a_directory_name_and_is_treated_as_one() {
    // The id names a directory under the config directory. A name somebody types is not
    // allowed to decide where that directory is.
    assert_eq!(slug("Work"), "work");
    assert_eq!(slug("Work & Play"), "work-play");
    assert_eq!(slug("../../etc"), "etc");
    assert_eq!(slug("  "), "profile");
    assert_eq!(slug("/"), "profile");
    assert!(!slug("..").contains('.'));
}

#[test]
fn two_profiles_called_the_same_thing_get_different_directories() {
    let dir = config();
    let profiles = Profiles::load(dir.path().to_path_buf());
    let one = profiles.create("Work", GroupColour::Lane1);
    let two = profiles.create("Work", GroupColour::Lane2);

    assert_ne!(one, two);
    assert_ne!(profiles.session_path(&one), profiles.session_path(&two));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_repository_made_under_a_profile_carries_its_identity() {
    // The clone and create paths apply this without asking, because a repository that has just
    // been made has nothing of its own to overwrite. Everything else needs the button.
    use coral_app_lib::profile::{ProfileSettings, stamp_new_repository};
    use coral_core::testutil::TestRepo;

    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let settings = ProfileSettings {
        user: Identity {
            name: Some("Work Me".to_owned()),
            email: Some("me@work.example".to_owned()),
        },
        ..ProfileSettings::default()
    };

    stamp_new_repository(&settings, repo.path()).await;

    assert_eq!(
        repo.git(["config", "--local", "user.email"]),
        "me@work.example"
    );
    assert_eq!(repo.git(["config", "--local", "user.name"]), "Work Me");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_profile_that_sets_nothing_leaves_a_new_repository_alone() {
    // The default profile carries no identity, and a repository cloned under it must come out
    // exactly as git made it rather than with its author blanked.
    use coral_app_lib::profile::{ProfileSettings, stamp_new_repository};
    use coral_core::testutil::TestRepo;

    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let before = repo.git(["config", "--local", "user.email"]);

    stamp_new_repository(&ProfileSettings::default(), repo.path()).await;

    assert_eq!(repo.git(["config", "--local", "user.email"]), before);
}
