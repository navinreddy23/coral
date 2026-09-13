/**
 * A gallery of the panels that only appear after an interaction.
 *
 * Development only, and not part of the application bundle: `index.html` is the app's entry,
 * this is its own. It exists because the panels worth looking at — a populated detail panel, a
 * diff, a stopped merge — are three clicks into a repository, and there is no way to drive
 * those clicks in a headless browser without a debugging protocol.
 */
import { mount } from "svelte";

import "../src/styles/fonts.css";
import "../src/styles/tokens.css";
import "../src/styles/base.css";

import Details from "../src/app/Details.svelte";
import DiffView from "../src/app/DiffView.svelte";
import MergeTool from "../src/app/MergeTool.svelte";
import Palette from "../src/app/Palette.svelte";
import RebasePicker from "../src/app/RebasePicker.svelte";
import Ask from "../src/app/Ask.svelte";
import Menu from "../src/app/Menu.svelte";
import Splash from "../src/app/Splash.svelte";
import Start from "../src/app/Start.svelte";
import StatusBar from "../src/app/StatusBar.svelte";
import Toasts from "../src/app/Toasts.svelte";
import Toolbar from "../src/app/Toolbar.svelte";
import { StartState } from "../src/state/start.svelte";
import { ToastsState } from "../src/state/toasts.svelte";
import Preferences from "../src/app/Preferences.svelte";
import TabBar from "../src/app/TabBar.svelte";
import { TabsState } from "../src/state/tabs.svelte";
import { ThemeState } from "../src/state/theme.svelte";
import { ViewsState } from "../src/state/views.svelte";
import { ExperimentalState } from "../src/state/experimental.svelte";
import { ProfilesState } from "../src/state/profiles.svelte";
import { SigningState } from "../src/state/signing.svelte";
import { SshState } from "../src/state/ssh.svelte";
import { DiffState } from "../src/state/diff.svelte";
import { MergeState } from "../src/state/merge.svelte";
import { RebaseState } from "../src/state/rebase.svelte";
import { preview } from "../src/ipc/preview";
import type { CommitDetail, FileDiff } from "../src/ipc/types";

/** The same repository the fixture engine answers about, so the panels agree. */
const REPO = "/home/dev/projects/coral";

const root = document.getElementById("gallery");
if (!root) throw new Error("#gallery is missing");

const query = new URLSearchParams(location.search);

/*
 * The real theme state, told what to be by the query string.
 *
 * Setting `data-theme` by hand here used to work and no longer does: a `ThemeState` mounted
 * further down applies its own answer on construction and would take the attribute straight
 * back off again. Driving it through the state is also the honest test — this is the code the
 * window runs.
 */
const asked = query.get("theme");
const theme = new ThemeState();
theme.set(asked === "dark" || asked === "light" ? asked : "light");

/**
 * Which panels to render. The modal ones cover the page with their own backdrop, so they are
 * asked for one at a time rather than looked at through each other.
 */
const only = query.get("only");
const wanted = (title: string) =>
  only === null || title.toLowerCase().includes(only);

function panel(title: string, height: string): HTMLElement {
  const section = document.createElement("section");
  if (!wanted(title)) {
    // A detached target: the component mounts and is never shown.
    return document.createElement("div");
  }
  section.style.cssText = `margin: 0 0 20px; border: 1px solid var(--border); border-radius: 8px; overflow: hidden; background: var(--bg-0);`;
  const label = document.createElement("div");
  label.textContent = title;
  label.style.cssText = `font: 600 11px/1 system-ui; text-transform: uppercase; letter-spacing: .07em; color: var(--fg-2); padding: 8px 12px; background: var(--bg-1); border-bottom: 1px solid var(--border);`;
  const body = document.createElement("div");
  body.style.cssText = `height: ${height}; display: flex; position: relative;`;
  section.append(label, body);
  root!.append(section);
  return body;
}

root.style.cssText =
  "padding: 20px; background: var(--bg-1); min-height: 100%;";

// The commit panel, with a real commit and its files.
const detail = preview("commit_detail", { rev: "abc" }) as CommitDetail;
/** What every commit panel here shares, so a new prop is added in one place. */
const commitPanel = {
  detail,
  repo: REPO,
  compare: null,
  nothing: false,
  loading: false,
  error: null,
  onClearCompare: () => {},
  onOpenFile: () => {},
  onGrouping: () => {},
  onCopied: () => {},
};
mount(Details, {
  target: panel("Commit panel — path grouping", "360px"),
  props: { ...commitPanel, openPath: null, grouping: "path" as const },
});
mount(Details, {
  target: panel("Commit panel — tree grouping, a file open", "360px"),
  props: {
    ...commitPanel,
    openPath: "ui/src/graph/render.ts",
    grouping: "tree" as const,
  },
});

/*
 * The diff reads its layout out of the remembered view preferences rather than holding its
 * own, so each of these needs a `ViewsState` — and the two panels need different ones, since
 * inline and side by side is exactly the preference they differ on.
 */
const inlineViews = new ViewsState();
inlineViews.set("diff", "inline");
const inline = new DiffState(inlineViews);
inline.path = "ui/src/graph/render.ts";
inline.file = preview("file_diff", {}) as FileDiff;
mount(DiffView, {
  target: panel("Diff — inline", "260px"),
  props: { diff: inline, onClose: () => {}, onPart: () => {} },
});

const splitViews = new ViewsState();
splitViews.set("diff", "split");
const split = new DiffState(splitViews);
split.path = "ui/src/graph/render.ts";
split.file = preview("file_diff", {}) as FileDiff;
mount(DiffView, {
  target: panel("Diff — side by side", "260px"),
  props: { diff: split, onClose: () => {}, onPart: () => {} },
});

const merge = new MergeState();
merge.operation = {
  state: "rebase",
  labels: { ours: "master", theirs: "graph-lanes", swapped: true },
  progress: { current: 2, total: 5 },
  headName: "graph-lanes",
  stoppedAt: null,
  interactive: true,
  resumable: true,
  applying: false,
  prepared: null,
};
merge.files = [
  {
    path: "ui/src/graph/render.ts",
    kind: "both_modified",
    binary: false,
    deleteModify: false,
    lfs: false,
  },
  {
    path: "assets/logo.png",
    kind: "both_added",
    binary: true,
    deleteModify: false,
    lfs: false,
  },
];
merge.active = "ui/src/graph/render.ts";
merge.blocks = {
  blocks: [
    {
      kind: "common",
      lines: ["function drawThroughLanes(", "  ctx: CanvasRenderingContext2D,"],
    },
    {
      kind: "conflict",
      base: ["  const open = new Map();"],
      ours: [
        "  const open = new Map<number, number>();",
        "  const entering = frame.open[first] ?? 0;",
      ],
      theirs: ["  const open = new Map<number, number>();"],
    },
    {
      kind: "common",
      lines: [
        "  for (let row = window.first; row <= window.last; row++) {",
        "  }",
      ],
    },
  ],
};
mount(MergeTool, {
  target: panel("Merge tool — a rebase stopped on a conflict", "380px"),
  props: { merge, onDone: () => {} },
});

const rebase = new RebaseState();
rebase.onto = "master";
rebase.items = [
  {
    step: "pick",
    oid: "a".repeat(40),
    summary: "graph: seed the open-lane mask",
    message: null,
  },
  {
    step: "reword",
    oid: "b".repeat(40),
    summary: "core: fix the credential fallback",
    message: "core: answer a request that carries no username",
  },
  {
    step: "squash",
    oid: "c".repeat(40),
    summary: "fixup: a typo",
    message: null,
  },
  {
    step: "drop",
    oid: "d".repeat(40),
    summary: "wip: scratch work",
    message: null,
  },
];
mount(RebasePicker, {
  target: panel("Interactive rebase", "340px"),
  props: { rebase, onDone: () => {} },
});

mount(Palette, {
  target: panel("Command palette", "340px"),
  props: {
    commands: [
      { id: "1", label: "Push", group: "Remote", run: () => {} },
      {
        id: "2",
        label: "Pull (fast-forward only)",
        group: "Remote",
        run: () => {},
      },
      {
        id: "3",
        label: "Checkout graph-lanes",
        group: "Branch",
        run: () => {},
      },
      {
        id: "4",
        label: "Merge graph-lanes into master",
        group: "Branch",
        run: () => {},
      },
      {
        id: "5",
        label: "Rebase onto master, interactively",
        group: "Branch",
        run: () => {},
      },
      { id: "6", label: "Stash changes", group: "Stash", run: () => {} },
    ],
    onClose: () => {},
  },
});

/*
 * The tab bar mid-drag. The highlight only exists while a pointer is down, so the events are
 * dispatched here rather than waited for: this is the one state of the bar nobody can
 * photograph by holding still.
 */
const tabs = new TabsState();
tabs.session = {
  tabs: [
    { id: 1, path: "/repos/coral", submodule: null, group: 5, missing: false },
    { id: 2, path: "/repos/linux", submodule: null, group: 5, missing: false },
    {
      id: 3,
      path: "/repos/notes",
      submodule: null,
      group: null,
      missing: false,
    },
    {
      id: 4,
      path: "/repos/zephyr",
      submodule: null,
      group: null,
      missing: false,
    },
  ],
  groups: [{ id: 5, name: "work", colour: "lane1", collapsed: false }],
  active: 1,
};
const barTarget = panel("Tab bar — dragging a loose tab onto a group", "70px");
mount(TabBar, {
  target: barTarget,
  props: {
    tabs,
    newTab: false,
    onOpen: () => {},
    onCloseNew: () => {},
    onPick: () => {},
    onAsk: async () => null,
  },
});
queueMicrotask(() => {
  const chips = [...barTarget.querySelectorAll(".tab")] as HTMLElement[];
  const band = barTarget.querySelector(".band");
  const dragged = chips.find((c) => c.textContent?.includes("notes"));
  if (!dragged || !band) return;
  const transfer = { setData: () => {}, effectAllowed: "", dropEffect: "" };
  const fire = (el: Element, type: string) =>
    el.dispatchEvent(
      Object.assign(new Event(type, { bubbles: true, cancelable: true }), {
        dataTransfer: transfer,
      }),
    );
  fire(dragged, "dragstart");
  fire(band, "dragover");
});

const signing = new SigningState();
signing.scopes = {
  effective: {
    format: "openpgp",
    program: "gpg",
    key: "0633C12121B1A10FADE103E39E9BC1B3B7C4AA29",
    signCommits: true,
    signTags: false,
  },
  global: {
    format: "openpgp",
    program: "",
    key: "AAAA111122223333444455556666777788889999",
    signCommits: false,
    signTags: false,
  },
  local: {
    format: null,
    program: "gpg",
    key: "0633C12121B1A10FADE103E39E9BC1B3B7C4AA29",
    signCommits: true,
    signTags: null,
  },
};
signing.keys = [
  {
    id: "0633C12121B1A10FADE103E39E9BC1B3B7C4AA29",
    label: "Ada Lovelace <ada@work.example>",
    expires: 1_851_575_560n,
    expired: false,
  },
  {
    id: "AAAA111122223333444455556666777788889999",
    label: "Ada Lovelace <ada@personal.example>",
    expires: null,
    expired: false,
  },
  {
    id: "BBBB111122223333444455556666777788889999",
    label: "Old Laptop <old@personal.example>",
    expires: 1_600_000_000n,
    expired: true,
  },
];
mount(Preferences, {
  target: panel(
    "Preferences — commit signing, overridden by this repository",
    "560px",
  ),
  props: {
    signing,
    ssh: new SshState(),
    experimental: new ExperimentalState(),
    profiles: new ProfilesState(),
    theme,
    views: new ViewsState(),
    identity: null,
    pane: "signing",
    repository: REPO,
    hasRepository: true,
    onClose: () => {},
    onCopied: () => {},
    onPickGit: () => {},
    onSwitchProfile: () => {},
    onDeleteProfile: () => {},
    onApplyProfileHere: () => {},
  },
});

mount(Ask, {
  target: panel("Asking a question", "300px"),
  props: {
    title: "New branch",
    detail: "Created at master and checked out.",
    placeholder: "feature/…",
    initial: "feature/lane-mask",
    choices: [{ id: "create", label: "Create branch", primary: true }],
    onAnswer: () => {},
  },
});

/*
 * The rest of the window.
 *
 * Everything above is a panel that appears after an interaction. What follows is the screens
 * and the chrome — the parts that are always on show, and which until now could only be looked
 * at by opening a repository and taking a picture of the whole window. They are here so that a
 * change to one of them can be seen on its own, in both themes, without the graph behind it.
 */

const start = new StartState();
const NOW = Math.floor(Date.now() / 1000);
start.recents = [
  { path: "/home/dev/projects/coral", name: "coral", opened: NOW - 600, missing: false },
  { path: "/home/dev/projects/linux", name: "linux", opened: NOW - 86_400, missing: false },
  { path: "/home/dev/projects/notes", name: "notes", opened: NOW - 9 * 86_400, missing: true },
];
mount(Start, {
  target: panel("Start page", "460px"),
  props: {
    start,
    sshKeys: [
      {
        path: "/home/dev/.ssh/id_ed25519",
        publicPath: "/home/dev/.ssh/id_ed25519.pub",
        comment: "dev@example",
        kind: "ed25519",
      },
    ],
    defaultSshKey: "/home/dev/.ssh/id_ed25519",
    onClose: () => {},
    onOpen: () => 1,
    onConfirm: async () => true,
    onPickDirectory: async () => null,
  },
});

const cloning = new StartState();
cloning.form = "clone";
mount(Start, {
  target: panel("Start page — cloning", "520px"),
  props: {
    start: cloning,
    sshKeys: [
      {
        path: "/home/dev/.ssh/id_ed25519",
        publicPath: "/home/dev/.ssh/id_ed25519.pub",
        comment: "dev@example",
        kind: "ed25519",
      },
    ],
    defaultSshKey: "/home/dev/.ssh/id_ed25519",
    onClose: () => {},
    onOpen: () => 1,
    onConfirm: async () => true,
    onPickDirectory: async () => null,
  },
});

mount(Splash, {
  target: panel("Loading screen", "300px"),
  props: { repo: "coral", path: REPO, stage: "walking" },
});

mount(Toolbar, {
  target: panel("Toolbar", "60px"),
  props: {
    repo: "coral",
    path: REPO,
    submodule: null,
    branch: "master",
    busy: false,
    comparing: false,
    terminalOpen: false,
    stashes: 2,
    dirty: true,
    journal: { undo: null, redo: null },
    leftPanel: "open",
    rightPanel: true,
    rightPanelUsable: true,
    onAction: () => {},
    onLeaveSubmodule: () => {},
    onPullMenu: () => {},
    onPushMenu: () => {},
  },
});

mount(StatusBar, {
  target: panel("Status bar", "44px"),
  props: {
    branch: "master",
    head: undefined,
    commits: 1_481_528,
    changed: 3,
    gitVersion: "2.43.0",
    host: {
      host: {
        kind: "github",
        origin: "github.com",
        owner: "coral",
        repo: "coral",
      },
      detail: null,
      token: "none",
    },
    report: null,
    busy: false,
    onDismiss: () => {},
    onLogs: () => {},
    onHost: () => {},
  },
});

const toasts = new ToastsState();
toasts.push("ok", "Pushed master to origin", "3 commits");
toasts.push("warn", "The remote has moved on", "Pull before pushing again.");
toasts.push(
  "error",
  "Could not fetch upstream",
  "Host key verification failed.",
);
mount(Toasts, { target: panel("Toasts", "190px"), props: { toasts } });

mount(Menu, {
  target: panel("Context menu", "360px"),
  props: {
    x: 24,
    y: 24,
    items: [
      { kind: "item", label: "Checkout this commit", run: () => {} },
      {
        kind: "item",
        label: "Create branch here",
        hint: "Ctrl B",
        run: () => {},
      },
      { kind: "separator" },
      {
        kind: "submenu",
        label: "Reset master to this commit",
        items: [
          { kind: "item", label: "Soft", run: () => {} },
          { kind: "item", label: "Mixed", checked: true, run: () => {} },
          { kind: "item", label: "Hard", danger: true, run: () => {} },
        ],
      },
      { kind: "item", label: "Revert commit", run: () => {} },
      { kind: "separator" },
      { kind: "item", label: "Drop commit", danger: true, run: () => {} },
    ],
    onClose: () => {},
  },
});
