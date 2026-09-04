/**
 * A gallery of the panels that only appear after an interaction.
 *
 * Development only, and not part of the application bundle: `index.html` is the app's entry,
 * this is its own. It exists because the panels worth looking at — a populated detail panel, a
 * diff, a stopped merge — are three clicks into a repository, and there is no way to drive
 * those clicks in a headless browser without a debugging protocol.
 */
import { mount } from 'svelte';

import '../src/styles/tokens.css';
import '../src/styles/base.css';

import Details from '../src/app/Details.svelte';
import DiffView from '../src/app/DiffView.svelte';
import MergeTool from '../src/app/MergeTool.svelte';
import Palette from '../src/app/Palette.svelte';
import RebasePicker from '../src/app/RebasePicker.svelte';
import Ask from '../src/app/Ask.svelte';
import Preferences from '../src/app/Preferences.svelte';
import { SigningState } from '../src/state/signing.svelte';
import { DiffState } from '../src/state/diff.svelte';
import { MergeState } from '../src/state/merge.svelte';
import { RebaseState } from '../src/state/rebase.svelte';
import { preview } from '../src/ipc/preview';
import type { CommitDetail, FileDiff } from '../src/ipc/types';

const root = document.getElementById('gallery');
if (!root) throw new Error('#gallery is missing');

const query = new URLSearchParams(location.search);
document.documentElement.dataset['theme'] = query.get('theme') ?? 'light';

/**
 * Which panels to render. The modal ones cover the page with their own backdrop, so they are
 * asked for one at a time rather than looked at through each other.
 */
const only = query.get('only');
const wanted = (title: string) => only === null || title.toLowerCase().includes(only);

function panel(title: string, height: string): HTMLElement {
  const section = document.createElement('section');
  if (!wanted(title)) {
    // A detached target: the component mounts and is never shown.
    return document.createElement('div');
  }
  section.style.cssText = `margin: 0 0 20px; border: 1px solid var(--border); border-radius: 8px; overflow: hidden; background: var(--bg-0);`;
  const label = document.createElement('div');
  label.textContent = title;
  label.style.cssText = `font: 600 11px/1 system-ui; text-transform: uppercase; letter-spacing: .07em; color: var(--fg-2); padding: 8px 12px; background: var(--bg-1); border-bottom: 1px solid var(--border);`;
  const body = document.createElement('div');
  body.style.cssText = `height: ${height}; display: flex; position: relative;`;
  section.append(label, body);
  root!.append(section);
  return body;
}

root.style.cssText = 'padding: 20px; background: var(--bg-1); min-height: 100%;';

// The commit panel, with a real commit and its files.
const detail = preview('commit_detail', { rev: 'abc' }) as CommitDetail;
mount(Details, {
  target: panel('Commit panel — path grouping', '360px'),
  props: { detail, loading: false, error: null, openPath: null, onOpenFile: () => {} },
});
mount(Details, {
  target: panel('Commit panel — tree grouping, a file open', '360px'),
  props: {
    detail,
    loading: false,
    error: null,
    openPath: 'ui/src/graph/render.ts',
    onOpenFile: () => {},
  },
});

const inline = new DiffState();
inline.path = 'ui/src/graph/render.ts';
inline.file = preview('file_diff', {}) as FileDiff;
mount(DiffView, { target: panel('Diff — inline', '260px'), props: { diff: inline, onClose: () => {} } });

const split = new DiffState();
split.path = 'ui/src/graph/render.ts';
split.file = preview('file_diff', {}) as FileDiff;
split.mode = 'split';
mount(DiffView, { target: panel('Diff — side by side', '260px'), props: { diff: split, onClose: () => {} } });

const merge = new MergeState();
merge.operation = {
  state: 'rebase',
  labels: { ours: 'master', theirs: 'graph-lanes', swapped: true },
  progress: { current: 2, total: 5 },
  headName: 'graph-lanes',
  stoppedAt: null,
  interactive: true,
};
merge.files = [
  { path: 'ui/src/graph/render.ts', kind: 'both_modified', binary: false, deleteModify: false },
  { path: 'assets/logo.png', kind: 'both_added', binary: true, deleteModify: false },
];
merge.active = 'ui/src/graph/render.ts';
merge.blocks = {
  blocks: [
    { kind: 'common', lines: ['function drawThroughLanes(', '  ctx: CanvasRenderingContext2D,'] },
    {
      kind: 'conflict',
      base: ['  const open = new Map();'],
      ours: ['  const open = new Map<number, number>();', '  const entering = frame.open[first] ?? 0;'],
      theirs: ['  const open = new Map<number, number>();'],
    },
    { kind: 'common', lines: ['  for (let row = window.first; row <= window.last; row++) {', '  }'] },
  ],
};
mount(MergeTool, { target: panel('Merge tool — a rebase stopped on a conflict', '380px'), props: { merge, onDone: () => {} } });

const rebase = new RebaseState();
rebase.onto = 'master';
rebase.items = [
  { step: 'pick', oid: 'a'.repeat(40), summary: 'graph: seed the open-lane mask', message: null },
  { step: 'reword', oid: 'b'.repeat(40), summary: 'core: fix the credential fallback', message: 'core: answer a request that carries no username' },
  { step: 'squash', oid: 'c'.repeat(40), summary: 'fixup: a typo', message: null },
  { step: 'drop', oid: 'd'.repeat(40), summary: 'wip: scratch work', message: null },
];
mount(RebasePicker, { target: panel('Interactive rebase', '340px'), props: { rebase, onDone: () => {} } });

mount(Palette, {
  target: panel('Command palette', '340px'),
  props: {
    commands: [
      { id: '1', label: 'Push', group: 'Remote', run: () => {} },
      { id: '2', label: 'Pull (fast-forward only)', group: 'Remote', run: () => {} },
      { id: '3', label: 'Checkout graph-lanes', group: 'Branch', run: () => {} },
      { id: '4', label: 'Merge graph-lanes into master', group: 'Branch', run: () => {} },
      { id: '5', label: 'Rebase onto master, interactively', group: 'Branch', run: () => {} },
      { id: '6', label: 'Stash changes', group: 'Stash', run: () => {} },
    ],
    onClose: () => {},
  },
});

const signing = new SigningState();
signing.scopes = {
  effective: {
    format: 'openpgp',
    program: 'gpg',
    key: '0633C12121B1A10FADE103E39E9BC1B3B7C4AA29',
    signCommits: true,
    signTags: false,
  },
  global: {
    format: 'openpgp',
    program: '',
    key: 'AAAA111122223333444455556666777788889999',
    signCommits: false,
    signTags: false,
  },
  local: {
    format: null,
    program: 'gpg',
    key: '0633C12121B1A10FADE103E39E9BC1B3B7C4AA29',
    signCommits: true,
    signTags: null,
  },
};
signing.keys = [
  {
    id: '0633C12121B1A10FADE103E39E9BC1B3B7C4AA29',
    label: 'Navin Reddy <navin@work.example>',
    expires: 1_851_575_560,
    expired: false,
  },
  {
    id: 'AAAA111122223333444455556666777788889999',
    label: 'Navin Reddy <navin@personal.example>',
    expires: null,
    expired: false,
  },
  {
    id: 'BBBB111122223333444455556666777788889999',
    label: 'Old Laptop <old@personal.example>',
    expires: 1_600_000_000,
    expired: true,
  },
];
mount(Preferences, {
  target: panel('Preferences — commit signing, overridden by this repository', '560px'),
  props: { signing, onClose: () => {} },
});

mount(Ask, {
  target: panel('Asking a question', '300px'),
  props: {
    title: 'New branch',
    detail: 'Created at master and checked out.',
    placeholder: 'feature/…',
    initial: 'feature/lane-mask',
    choices: [{ id: 'create', label: 'Create branch', primary: true }],
    onAnswer: () => {},
  },
});
