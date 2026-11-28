//! The graph of real repositories, built with git and drawn as box art.
//!
//! `lanes.rs` feeds the assigner DAGs written out by hand; these go through git, the walk and
//! the row store, so the order the walk really emits — a merged branch before the mainline it
//! joined, a stash on its commit, an orphan beside the history it does not share — is the one
//! laid out. Every picture is checked against the repository's own parents before it is
//! compared.

mod common;

use std::collections::HashMap;

use common::assert_drawable;
use coral_core::graph::{GixCommitStream, NO_ROW, RowStore, RowTopology, StreamOpts, art, build};
use coral_core::testutil::TestRepo;

/// A repository whose commits are an hour apart, as a real one's are, so the walk's order is
/// the one a history made by people would get rather than one decided by equal timestamps.
struct History {
    repo: TestRepo,
    hour: u32,
}

impl History {
    fn new() -> Self {
        let mut h = Self {
            repo: TestRepo::new(),
            hour: 0,
        };
        h.commit("base");
        h
    }

    /// Runs git at the next hour.
    fn at(&mut self, args: &[&str]) -> &mut Self {
        self.hour += 1;
        let when = format!("@{} +0000", 1_700_000_000 + u64::from(self.hour) * 3600);
        let out = self
            .repo
            .command(args)
            .env("GIT_AUTHOR_DATE", &when)
            .env("GIT_COMMITTER_DATE", &when)
            .output()
            .expect("spawn git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        self
    }

    fn commit(&mut self, message: &str) -> &mut Self {
        self.at(&["commit", "--quiet", "--allow-empty", "-m", message])
    }

    fn merge(&mut self, message: &str, branches: &[&str]) -> &mut Self {
        let mut args = vec!["merge", "--quiet", "--no-ff", "-m", message];
        args.extend_from_slice(branches);
        self.at(&args)
    }

    fn branch(&mut self, name: &str) -> &mut Self {
        self.repo.git(["checkout", "--quiet", "-b", name]);
        self
    }

    fn switch(&mut self, name: &str) -> &mut Self {
        self.repo.git(["checkout", "--quiet", name]);
        self
    }

    /// The graph as the window would lay it out, as a picture labelled with each commit's
    /// subject, after checking every line in it ends at the commit it was drawn for.
    fn picture(&self) -> String {
        let stream = GixCommitStream::open(self.repo.path()).expect("open");
        let store = build(&stream, &StreamOpts::default()).expect("walk");
        let (rows, dag) = topology(&store);
        assert_drawable(&dag, &rows, true);

        let subjects = self.subjects();
        let labels: Vec<String> = (0..store.len())
            .map(|row| {
                let oid = store.oid(row).expect("oid").to_string();
                subjects
                    .get(&oid)
                    .cloned()
                    .unwrap_or_else(|| "?".to_owned())
            })
            .collect();
        let drawn: Vec<art::Row<'_>> = rows.iter().map(art::Row::from).collect();
        common::labelled(&art::draw(&drawn), labels.iter().map(String::as_str))
    }

    fn subjects(&self) -> HashMap<String, String> {
        self.repo
            .git(["log", "--all", "--format=%H %s"])
            .lines()
            .filter_map(|l| l.split_once(' '))
            .map(|(oid, subject)| (oid.to_owned(), subject.to_owned()))
            .collect()
    }
}

/// The store's rows as the assigner gave them, and each row's parents as rows.
fn topology(store: &RowStore) -> (Vec<RowTopology>, Vec<Vec<usize>>) {
    (0..store.len())
        .map(|row| {
            let topo = RowTopology {
                lane: store.lane(row).expect("lane"),
                parent_lanes: store.parent_lanes(row).iter().copied().collect(),
                width: 0,
                open: store.open(row),
                joins: store.joins(row),
            };
            let parents = store
                .parents(row)
                .iter()
                .map(|&p| {
                    assert_ne!(p, NO_ROW, "row {row} has a parent outside the walk");
                    p as usize
                })
                .collect();
            (topo, parents)
        })
        .unzip()
}

#[test]
fn branches_merged_one_after_another_keep_main_straight() {
    let mut h = History::new();
    h.commit("main: one");
    h.branch("feature/a").commit("a: one").commit("a: two");
    h.switch("main").commit("main: two");
    h.branch("feature/b").commit("b: one");
    h.switch("main").merge("merge a", &["feature/a"]);
    h.branch("feature/c").commit("c: one").commit("c: two");
    h.switch("feature/b").commit("b: two");
    h.switch("main")
        .merge("merge b", &["feature/b"])
        .commit("main: three");
    h.merge("merge c", &["feature/c"]);
    h.switch("main")
        .branch("feature/d")
        .commit("d: one, not merged");
    insta::assert_snapshot!(h.picture());
}

#[test]
fn a_branch_off_a_branch_turns_into_the_branch_it_left() {
    let mut h = History::new();
    h.branch("feature")
        .commit("feature: one")
        .commit("feature: two");
    h.branch("feature-sub")
        .commit("sub: one")
        .commit("sub: two");
    h.switch("feature").commit("feature: three");
    h.merge("merge sub into feature", &["feature-sub"]);
    h.switch("main").commit("main: one");
    h.merge("merge feature", &["feature"]);
    insta::assert_snapshot!(h.picture());
}

#[test]
fn main_merged_into_a_feature_and_the_feature_back() {
    let mut h = History::new();
    h.branch("feature").commit("feature: one");
    h.switch("main").commit("main: one").commit("main: two");
    h.switch("feature")
        .merge("merge main into feature", &["main"]);
    h.commit("feature: two");
    h.switch("main").commit("main: three");
    h.merge("merge feature", &["feature"]);
    insta::assert_snapshot!(h.picture());
}

#[test]
fn an_octopus_merge_fans_in_from_the_right() {
    let mut h = History::new();
    for name in ["x", "y", "z"] {
        h.switch("main")
            .branch(name)
            .commit(&format!("{name}: one"));
    }
    h.switch("main").commit("main: one");
    h.merge("octopus", &["x", "y", "z"]);
    insta::assert_snapshot!(h.picture());
}

#[test]
fn a_criss_cross_merge_joins_the_lines_already_running() {
    let mut h = History::new();
    h.branch("x").commit("x: one");
    h.switch("main").branch("y").commit("y: one");
    h.switch("x").merge("x takes y", &["y"]);
    h.switch("y")
        .at(&["merge", "--quiet", "--no-ff", "-m", "y takes x", "x~1"]);
    h.switch("main")
        .merge("main takes x", &["x"])
        .merge("main takes y", &["y"]);
    insta::assert_snapshot!(h.picture());
}

/// An orphan is walked after the history it does not share, as `git log --topo-order` walks it:
/// below main's root, in the lane main's root left free, with nothing joining the two.
#[test]
fn an_orphan_branch_is_drawn_after_the_history_it_does_not_share() {
    let mut h = History::new();
    h.commit("main: one");
    h.repo.git(["checkout", "--quiet", "--orphan", "gh-pages"]);
    h.commit("pages: root").commit("pages: two");
    h.switch("main").commit("main: two");
    h.switch("gh-pages").commit("pages: three");
    h.switch("main").commit("main: three");
    insta::assert_snapshot!(h.picture());
}

#[test]
fn an_unrelated_history_merged_in_ends_at_its_own_root() {
    let mut h = History::new();
    h.commit("main: one");
    h.repo.git(["checkout", "--quiet", "--orphan", "imported"]);
    h.commit("imported: root").commit("imported: two");
    h.switch("main").commit("main: two");
    h.at(&[
        "merge",
        "--quiet",
        "--no-ff",
        "--allow-unrelated-histories",
        "-m",
        "merge imported",
        "imported",
    ]);
    insta::assert_snapshot!(h.picture());
}

#[test]
fn a_stash_and_a_tag_sit_on_the_lines_they_belong_to() {
    let mut h = History::new();
    h.commit("main: one");
    h.at(&["tag", "-a", "-m", "v1", "v1"]);
    h.branch("feature").commit("feature: one");
    h.switch("main").commit("main: two");
    h.repo = h.repo.write("notes.txt", "half written\n");
    h.repo.git(["add", "notes.txt"]);
    h.at(&["stash", "push", "--quiet", "-m", "notes"]);
    insta::assert_snapshot!(h.picture());
}
