/**
 * A fixture engine, for running the window in an ordinary browser.
 *
 * Development only. Vite replaces `import.meta.env.DEV` with `false` for a release build, so
 * every reference below is dead code the bundler drops — none of this ships. Its purpose is to
 * make the interface reviewable without a Tauri shell: in a browser there are devtools, and
 * there is a way to take a picture of it, neither of which WebKitGTK offers.
 *
 * The data is shaped like a real repository rather than minimal: several lanes, merges, refs
 * of every kind, a dirty worktree and open proposals, because a screen with one of everything
 * is the one worth looking at.
 */
import { MAGIC, Section, VERSION } from '../graph/frame';

const REPO = '/home/dev/projects/coral';

interface Row {
  lane: number;
  parents: number[];
  merge: boolean;
  summary: string;
  author: string;
  email: string;
}

const AUTHORS: [string, string][] = [
  ['Ada Lovelace', 'ada@example.com'],
  ['Grace Hopper', 'grace@example.com'],
  ['Alan Turing', 'alan@example.com'],
  ['Katherine Johnson', 'katherine@example.com'],
  ['Linus Torvalds', 'torvalds@example.com'],
];

const SUMMARIES = [
  'graph: keep a lane drawn across the whole span it is open for',
  'core: answer a credential request that carries no username',
  'ui: page the graph instead of holding only its first frame',
  'app: open a file diff from the commit panel, inline or side by side',
  'test: mount the components and check what they render',
  'core: hold sixteen times more rows in a frame',
  'app: a three-way merge tool for conflicts',
  'docs: record why conflict blocks are rebuilt from index stages',
  'ui: stop WebKitGTK dropping text to grayscale antialiasing',
  'hosting: read pull and merge requests from GitHub and GitLab',
];

/**
 * A history with side branches that open, run, and merge back.
 *
 * Written out as lanes per row rather than generated, so the shape is one a real repository
 * would have: every branch has a merge above it and a fork below, which is what a lane
 * renderer has to get right and what a modular pattern never quite produces.
 */
function rows(): Row[] {
  // 0 = mainline, 1 and 2 side branches. A row that returns to 0 from a side branch merges.
  const lanes = [
    0, 0, 1, 1, 1, 0, 0, 0, 2, 2, 2, 2, 0, 0, 1, 1, 0, 0, 0, 0,
    1, 1, 1, 0, 0, 2, 2, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 2, 2, 2,
    0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 1, 0, 0, 2, 2, 0, 0, 0, 0, 0,
  ];
  return lanes.map((lane, i) => {
    const next = lanes[i + 1] ?? 0;
    // A mainline commit whose parent is on a side branch is where that branch was merged in.
    const merge = lane === 0 && next !== 0;
    const author = AUTHORS[i % AUTHORS.length] ?? AUTHORS[0]!;
    return {
      lane,
      parents: merge ? [0, next] : [next === lane ? lane : next],
      merge,
      summary: SUMMARIES[i % SUMMARIES.length] ?? 'a commit',
      author: author[0],
      email: author[1],
    };
  });
}

const ROWS = rows();

function oidOf(row: number): string {
  // Deterministic and hex, so it reads like an object id in the interface.
  let hash = 0x811c9dc5;
  for (const ch of `commit-${row}`) {
    hash ^= ch.charCodeAt(0);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash.toString(16).padStart(8, '0').repeat(5);
}

/** Mirrors `graph::wire::encode`, checked by the decoder the window already uses. */
function frame(startRow: number): ArrayBuffer {
  const HEADER = 24;
  const SECTION = 16;
  const count = ROWS.length;
  const hashLen = 20;

  const lane = new Uint8Array(count * 2);
  const laneView = new DataView(lane.buffer);
  const flags = new Uint8Array(count);
  const time = new Uint8Array(count * 8);
  const timeView = new DataView(time.buffer);
  const open = new Uint8Array(count * 4);
  const openView = new DataView(open.buffer);
  const parentLanes: number[] = [];
  const parentStart = new Uint8Array((count + 1) * 4);
  const startView = new DataView(parentStart.buffer);
  const oids = new Uint8Array(count * hashLen);

  const now = Math.floor(Date.now() / 1000);
  let edges = 0;
  startView.setUint32(0, 0, true);

  for (let i = 0; i < count; i++) {
    const row = ROWS[i]!;
    laneView.setUint16(i * 2, row.lane, true);
    flags[i] = row.merge ? 1 : 0;
    timeView.setFloat64(i * 8, now - i * 5400, true);

    // A lane is open entering this row when it has a commit above and another at or below:
    // that is what the engine's own mask means, and what keeps a run drawn end to end without
    // extending past either of its ends.
    let mask = 0;
    for (let l = 0; l <= 2; l++) {
      const above = ROWS.slice(0, i).some((r) => r.lane === l);
      const below = ROWS.slice(i).some((r) => r.lane === l);
      if (above && below) mask |= 1 << l;
    }
    openView.setUint32(i * 4, mask, true);

    for (const p of row.parents) {
      parentLanes.push(p);
      edges += 1;
    }
    startView.setUint32((i + 1) * 4, edges, true);

    const hex = oidOf(i);
    for (let b = 0; b < hashLen; b++) {
      oids[i * hashLen + b] = Number.parseInt(hex.slice(b * 2, b * 2 + 2), 16);
    }
  }

  const parentLane = new Uint8Array(parentLanes.length * 2);
  const plView = new DataView(parentLane.buffer);
  parentLanes.forEach((l, i) => plView.setUint16(i * 2, l, true));

  const sections: [number, Uint8Array, number][] = [
    [Section.Lane, lane, count],
    [Section.Flags, flags, count],
    [Section.Time, time, count],
    [Section.ParentStart, parentStart, count + 1],
    [Section.ParentLane, parentLane, parentLanes.length],
    [Section.Oid, oids, count],
    [Section.Open, open, count],
  ];

  const align8 = (n: number) => (n + 7) & ~7;
  const directory = HEADER + SECTION * sections.length;
  const offsets: number[] = [];
  let at = directory;
  for (const [, bytes] of sections) {
    at = align8(at);
    offsets.push(at);
    at += bytes.length;
  }

  const buffer = new ArrayBuffer(align8(at));
  const view = new DataView(buffer);
  const bytes = new Uint8Array(buffer);

  view.setUint32(0, MAGIC, true);
  view.setUint16(4, VERSION, true);
  view.setUint16(6, sections.length, true);
  view.setUint32(8, startRow, true);
  view.setUint32(12, count, true);
  view.setUint32(16, count, true);
  view.setUint8(20, 1); // final
  view.setUint8(21, hashLen);

  sections.forEach(([kind, payload, elems], i) => {
    const base = HEADER + i * SECTION;
    view.setUint32(base, kind, true);
    view.setUint32(base + 4, offsets[i]!, true);
    view.setUint32(base + 8, payload.length, true);
    view.setUint32(base + 12, elems, true);
    bytes.set(payload, offsets[i]!);
  });
  return buffer;
}

function selfTest(): ArrayBuffer {
  const out = new Uint8Array(4096);
  for (let i = 0; i < out.length; i++) out[i] = i % 251;
  return out.buffer;
}

function ref(short: string, kind: unknown, row: number, over: object = {}) {
  return {
    name: `refs/${short}`,
    short,
    kind,
    target: oidOf(row),
    peeled: null,
    upstream: null,
    ahead: 0,
    behind: 0,
    row,
    ...over,
  };
}

const TABS = {
  tabs: [
    { id: 1, path: REPO, submodule: null, group: 1 },
    { id: 2, path: '/home/dev/projects/linux', submodule: null, group: 1 },
    { id: 3, path: '/home/dev/projects/notes', submodule: null, group: null },
  ],
  active: 1,
  groups: [{ id: 1, name: 'work', colour: 'lane1', collapsed: false }],
};

/** The same session with the first tab stepped into its submodule, for the breadcrumb. */
const TABS_IN_SUBMODULE = {
  ...TABS,
  tabs: TABS.tabs.map((t) => (t.id === 1 ? { ...t, submodule: 'external/dev-scripts' } : t)),
};

const REMOTES = [
  {
    name: 'origin',
    fetchUrl: 'git@github.com:coral-dev/coral.git',
    pushUrl: 'git@github.com:coral-dev/coral.git',
  },
  {
    name: 'upstream',
    fetchUrl: 'https://gitlab.com/open-source-23/coral.git',
    pushUrl: 'https://gitlab.com/open-source-23/coral.git',
  },
];

const FIXTURES: Record<string, unknown> = {
  initial_repo: REPO,
  open_repo: {
    path: REPO,
    gitDir: `${REPO}/.git`,
    gitVersion: '2.43.0',
    head: { kind: 'branch', name: 'master' },
    state: 'clean',
    commitGraph: true,
  },
  session_get: TABS,
  tab_open: TABS,
  tab_activate: TABS,
  tab_close: TABS,
  tab_group: TABS,
  tab_ungroup: TABS,
  tab_move: TABS,
  group_collapse: TABS,
  group_rename: TABS,
  group_recolour: TABS,
  group_dissolve: TABS,
  group_close: TABS,
  tab_enter_submodule: TABS_IN_SUBMODULE,
  tab_leave_submodule: TABS,
  remote_list: REMOTES,
  remote_edit: REMOTES,
  profile_list: {
    profiles: [
      {
        id: 'personal',
        name: 'Personal',
        colour: 'lane1',
        settings: {
          user: { name: 'Ada Lovelace', email: 'ada@example.invalid' },
          ssh: { privateKey: null, publicKey: null, credentialHelper: null },
          signing: { format: null, program: null, key: null, signCommits: null, signTags: null },
        },
      },
      {
        id: 'work',
        name: 'Work',
        colour: 'lane3',
        settings: {
          user: { name: 'A. Lovelace', email: 'a.lovelace@analytical.invalid' },
          ssh: { privateKey: '/home/dev/.ssh/id_work', publicKey: null, credentialHelper: null },
          signing: { format: null, program: null, key: null, signCommits: null, signTags: null },
        },
      },
    ],
    current: 'personal',
  },
  repo_identity: {
    effective: { name: 'Ada Lovelace', email: 'ada@example.invalid' },
    global: { name: 'Ada Lovelace', email: 'ada@example.invalid' },
    local: { name: null, email: null },
  },
  ssh_read: {
    effective: {
      useAgent: false,
      privateKey: '/home/dev/.ssh/id_ed25519',
      publicKey: '/home/dev/.ssh/id_ed25519.pub',
      command: "ssh -F none -i '/home/dev/.ssh/id_ed25519' -o IdentitiesOnly=yes",
      credentialHelper: '',
    },
    global: {
      useAgent: true,
      privateKey: '',
      publicKey: '',
      command: '',
      credentialHelper: '',
    },
    local: {
      privateKey: '/home/dev/.ssh/id_ed25519',
      publicKey: '/home/dev/.ssh/id_ed25519.pub',
      credentialHelper: null,
    },
  },
  terminal_defaults: { shell: '/bin/zsh', login: false },
  ssh_keys: [
    {
      path: '/home/dev/.ssh/id_ed25519',
      publicPath: '/home/dev/.ssh/id_ed25519.pub',
      comment: 'dev@workstation',
      kind: 'ssh-ed25519',
    },
    {
      path: '/home/dev/.ssh/id_work',
      publicPath: '/home/dev/.ssh/id_work.pub',
      comment: 'dev@company',
      kind: 'ssh-ed25519',
    },
  ],
  ssh_public_key: 'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI0000000000 dev@workstation',
  submodule_revision: {
    oid: '3e88757dd4883b678cf708eaf79c02fc358cf18b',
    summary: 'Add backwards-compatibility with older QAPI version',
    time: 1_763_060_220,
    inSync: true,
  },
  watch_repo: { complete: true, detail: null },
  unwatch_repo: null,
  repo_refs: [
    ref('heads/master', { kind: 'local_branch' }, 0, {
      short: 'master',
      upstream: 'origin/master',
      ahead: 3,
      behind: 1,
    }),
    ref('heads/graph-lanes', { kind: 'local_branch' }, 2, { short: 'graph-lanes' }),
    ref('remotes/origin/master', { kind: 'remote_branch', remote: 'origin' }, 4, {
      short: 'origin/master',
    }),
    ref('remotes/origin/feature/audio-ringbuffer', { kind: 'remote_branch', remote: 'origin' }, 6, {
      short: 'origin/feature/audio-ringbuffer',
    }),
    ref('remotes/upstream/master', { kind: 'remote_branch', remote: 'upstream' }, 7, {
      short: 'upstream/master',
    }),
    ref('tags/v0.1.0', { kind: 'tag', annotated: true }, 9, { short: 'v0.1.0' }),
    ref('stash', { kind: 'stash' }, 14, { short: 'stash@{0}' }),
  ],
  repo_submodules: [
    {
      name: 'external/dev-scripts',
      path: 'external/dev-scripts',
      url: 'https://example.com/dev-scripts.git',
      pinned: oidOf(3),
      initialised: true,
    },
  ],
  repo_status: {
    entries: [
      { path: 'ui/src/app/App.svelte', index: 'modified', worktree: 'unmodified' },
      { path: 'crates/coral-core/src/graph/lanes.rs', index: 'unmodified', worktree: 'modified' },
      { path: 'docs/ARCHITECTURE.md', index: 'unmodified', worktree: 'modified' },
    ],
    conflicted: [],
  },
  repo_operation: {
    state: 'clean',
    labels: { ours: 'master', theirs: '', swapped: false },
    progress: null,
    headName: null,
    stoppedAt: null,
    interactive: false,
  },
  repo_conflicts: [],
  hosting_status: {
    host: { kind: 'gitlab', origin: 'https://gitlab.com', owner: 'open-source-23', repo: 'coral' },
    detail: null,
    signedIn: true,
  },
  hosting_pull_requests: [
    {
      number: 42,
      title: 'Keep a lane drawn across the whole span it is open for',
      state: 'open',
      author: 'ada',
      sourceBranch: 'graph-lanes',
      targetBranch: 'master',
      webUrl: 'https://gitlab.com/x/-/merge_requests/42',
      updatedAt: '2026-09-01T10:00:00Z',
    },
    {
      number: 41,
      title: 'Page the graph instead of holding only its first frame',
      state: 'merged',
      author: 'grace',
      sourceBranch: 'paging',
      targetBranch: 'master',
      webUrl: 'https://gitlab.com/x/-/merge_requests/41',
      updatedAt: '2026-08-30T09:00:00Z',
    },
    {
      number: 40,
      title: 'Draft: interactive rebase',
      state: 'draft',
      author: 'alan',
      sourceBranch: 'rebase-i',
      targetBranch: 'master',
      webUrl: 'https://gitlab.com/x/-/merge_requests/40',
      updatedAt: '2026-08-28T09:00:00Z',
    },
  ],
};

function metadata(startRow: number, count: number): unknown[] {
  const now = Math.floor(Date.now() / 1000);
  return Array.from({ length: count }, (_, i) => {
    const at = startRow + i;
    const row = ROWS[at % ROWS.length]!;
    return {
      oid: oidOf(at),
      author: row.author,
      email: row.email,
      time: now - at * 5400,
      summary: row.summary,
      body: at % 4 === 0 ? 'Found by running the thing rather than reasoning about it.' : '',
    };
  });
}

function detail(rev: string) {
  const row = ROWS[0]!;
  return {
    commit: {
      oid: rev,
      parents: [oidOf(1)],
      author: { name: row.author, email: row.email, time: Math.floor(Date.now() / 1000) },
      committer: { name: row.author, email: row.email, time: Math.floor(Date.now() / 1000) },
      summary: row.summary,
      body: 'A lane reserved for a parent stays occupied until that parent’s own row.\n\nOn the kernel that span is routinely hundreds of thousands of rows.',
    },
    files: [
      { path: 'ui/src/graph/render.ts', oldPath: null, change: 'modified' },
      { path: 'crates/coral-core/src/graph/lanes.rs', oldPath: null, change: 'modified' },
      { path: 'ui/tests/render.test.ts', oldPath: null, change: 'added' },
      { path: 'docs/ARCHITECTURE.md', oldPath: null, change: 'modified' },
      { path: 'ui/src/graph/old-render.ts', oldPath: null, change: 'deleted' },
    ],
  };
}

const DIFF = {
  path: 'ui/src/graph/render.ts',
  oldPath: null,
  change: 'modified',
  binary: false,
  added: 3,
  removed: 1,
  tooLarge: false,
  hunks: [
    {
      header: '@@ -96,8 +96,10 @@ function drawThroughLanes(',
      oldStart: 96,
      oldLines: 8,
      newStart: 96,
      newLines: 10,
      lines: [
        { kind: 'context', text: '  const open = new Map<number, number>();', oldNo: 96, newNo: 96, noNewline: false },
        { kind: 'remove', text: '  // Discovered by scanning the window.', oldNo: 97, newNo: null, noNewline: false },
        { kind: 'add', text: '  // Seeded from the frame’s per-row open mask.', oldNo: null, newNo: 97, noNewline: false },
        { kind: 'add', text: '  const entering = frame.open[window.first - frame.startRow] ?? 0;', oldNo: null, newNo: 98, noNewline: false },
        { kind: 'add', text: '  for (let lane = 0; lane < 32; lane++) {', oldNo: null, newNo: 99, noNewline: false },
        { kind: 'context', text: '    if ((entering & (1 << lane)) !== 0) open.set(lane, topY);', oldNo: 98, newNo: 100, noNewline: false },
        { kind: 'context', text: '  }', oldNo: 99, newNo: 101, noNewline: false },
      ],
    },
  ],
};

/**
 * Plays a short session into a terminal pane.
 *
 * There is no shell in a browser, and an empty black rectangle says nothing about whether the
 * pane works. This writes what one looks like a few seconds into use.
 */
export function previewTerminal(onData: (text: string) => void): () => void {
  const dim = '\u001b[2m';
  const off = '\u001b[0m';
  const green = '\u001b[32m';
  const yellow = '\u001b[33m';
  const cyan = '\u001b[36m';
  const prompt = `${green}dev${off}:${cyan}~/projects/coral${off}${yellow} master${off} $ `;

  const lines = [
    `${prompt}git status -sb\r\n`,
    `## ${green}master${off}...${dim}origin/master${off} [ahead 3, behind 1]\r\n`,
    ` M ui/src/app/TabBar.svelte\r\n`,
    ` M crates/coral-app/src/session.rs\r\n`,
    `?? crates/coral-app/tests/terminal.rs\r\n`,
    `${prompt}git log --oneline -3\r\n`,
    `${yellow}9e7702f${off} ui: finish the polish pass on the tabs and the diff\r\n`,
    `${yellow}1669548${off} ui: colour a commit node by its author\r\n`,
    `${yellow}733d9d5${off} core: answer a credential request that carries no username\r\n`,
    prompt,
  ];

  let at = 0;
  const timer = setInterval(() => {
    const line = lines[at];
    at += 1;
    if (line === undefined) {
      clearInterval(timer);
      return;
    }
    onData(line);
  }, 90);
  return () => clearInterval(timer);
}

/** Answers one command with fixture data. */
export function preview(command: string, args: Record<string, unknown>): unknown {
  switch (command) {
    case 'binary_self_test':
      return selfTest();
    case 'graph_frame':
      return frame(Number(args['startRow'] ?? 0));
    case 'row_metadata':
      return metadata(Number(args['startRow'] ?? 0), Number(args['count'] ?? 0));
    case 'commit_detail':
      return detail(String(args['rev'] ?? oidOf(0)));
    case 'file_diff':
      return DIFF;
    case 'worktree_diff':
      return DIFF;
    case 'commit_url':
      return `https://github.com/coral-dev/coral/commit/${String(args['oid'] ?? '')}`;
    case 'terminal_open':
      return { id: 1, shell: '/usr/bin/zsh' };
    case 'terminal_write':
    case 'terminal_resize':
    case 'terminal_close':
      return null;
    default:
      return FIXTURES[command] ?? null;
  }
}
