<script lang="ts">
  import { covers, hasFlag, localRow, oidOf, RowFlag, type Frame } from '../graph/frame';
  import GraphCanvas from '../graph/GraphCanvas.svelte';
  import { initialsOf } from '../graph/initials';
  import Splitter from './Splitter.svelte';
  import { PANE_LIMITS, PanesState } from '../state/panes.svelte';
  import DiffView from './DiffView.svelte';
  import { DiffState } from '../state/diff.svelte';
  import { HostingState } from '../state/hosting.svelte';
  import { openInBrowser, type PullRequest } from '../ipc/commands';
  import { ActionsState } from '../state/actions.svelte';
  import MergeTool from './MergeTool.svelte';
  import { MergeState } from '../state/merge.svelte';
  import RebasePicker from './RebasePicker.svelte';
  import { RebaseState } from '../state/rebase.svelte';
  import Palette, { type Command } from './Palette.svelte';
  import StatusBar from './StatusBar.svelte';
  import Ask, { type Choice } from './Ask.svelte';
  import Preferences from './Preferences.svelte';
  import Menu, { type MenuItem } from './Menu.svelte';
  import HostMark, { hostOf } from './HostMark.svelte';
  import Toasts from './Toasts.svelte';
  import Splash from './Splash.svelte';
  import Remotes from './Remotes.svelte';
  import SubmodulePanel from './Submodule.svelte';
  import { RemotesState } from '../state/remotes.svelte';
  import { describe, ToastsState } from '../state/toasts.svelte';
  import { copyText } from './clipboard';
  import { onRepoChanged, unwatchRepo, watchRepo, type RepoChanged } from '../ipc/watch';
  import Terminal from './Terminal.svelte';
  import { TerminalState } from '../state/terminal.svelte';
  import { SigningState } from '../state/signing.svelte';
  import { SshState } from '../state/ssh.svelte';
  import { elidePath, elideRef } from './path';
  import type { Action } from '../ipc/commands';
  import {
    DEFAULT_METRICS,
    firstRowFor,
    GRAPH_COLUMN_PX,
    listTop,
    REFS_COLUMN_PX,
    spacerHeight,
  } from '../graph/layout';
  import { commitUrl, initialRepo, open, pickDirectory, pickRepository } from '../ipc/commands';
  import { GraphState } from '../state/graph.svelte';
  import { RefsState } from '../state/refs.svelte';
  import { ThemeState } from '../state/theme.svelte';
  import { SelectionState } from '../state/selection.svelte';
  import { WorktreeState } from '../state/worktree.svelte';
  import Staging from './Staging.svelte';
  import Details from './Details.svelte';
  import Sidebar from './Sidebar.svelte';
  import { TabsState } from '../state/tabs.svelte';
  import { ViewsState } from '../state/views.svelte';
  import { isTextTarget, resolve, tabJump } from '../state/shortcuts';
  import Shortcuts from './Shortcuts.svelte';
  import TabBar from './TabBar.svelte';
  import Toolbar from './Toolbar.svelte';
  import type { RepoInfo, Submodule, SubmoduleRevision } from '../ipc/types';
  import { submoduleRevision } from '../ipc/commands';
  import type { PlacedRef } from '../state/refs.svelte';

  // Every remembered view choice lives here, so a toggle is a preference rather than a mode
  // the window forgets on the next launch.
  const views = new ViewsState();
  const graph = new GraphState();
  const theme = new ThemeState();
  const refs = new RefsState();
  const selection = new SelectionState();
  const worktree = new WorktreeState();
  let showWip = $state(false);
  const tabs = new TabsState();
  let showHelp = $state(false);

  /** Which shortcuts actually do something today; the help overlay dims the rest. */
  const LIVE = new Set([
    'select.next', 'select.previous', 'select.first', 'select.last',
    'stage.all', 'unstage.all', 'tab.new', 'tab.close', 'tab.next', 'tab.previous',
    'palette', 'repo.open', 'terminal',
    'panel.left', 'panel.detail', 'help',
  ]);

  function move(delta: number) {
    if (!graph.frame) return;
    const at = selection.row ?? -1;
    const next = Math.min(graph.totalRows - 1, Math.max(0, at + delta));
    pick(next);
    scrollToRow(next);
  }

  function onKey(event: KeyboardEvent) {
    const e = {
      key: event.key,
      ctrl: event.ctrlKey,
      shift: event.shiftKey,
      alt: event.altKey,
      meta: event.metaKey,
    };
    const jump = tabJump(e);
    if (jump !== null) {
      const target = tabs.session.tabs[jump - 1];
      if (target) void tabs.activate(target.id);
      event.preventDefault();
      return;
    }

    const binding = resolve(e, isTextTarget(event.target) ? 'message' : 'global');
    if (!binding || !LIVE.has(binding.id)) return;
    event.preventDefault();

    switch (binding.id) {
      case 'select.next': move(1); break;
      case 'select.previous': move(-1); break;
      case 'select.first': move(-Number.MAX_SAFE_INTEGER); break;
      case 'select.last': move(Number.MAX_SAFE_INTEGER); break;
      case 'stage.all': void worktree.stage(worktree.unstaged.map((f) => f.path), true); break;
      case 'unstage.all': void worktree.stage(worktree.staged.map((f) => f.path), false); break;
      case 'tab.new': void openAnother(); break;
      case 'tab.close': if (tabs.active) void tabs.close(tabs.active.id); break;
      case 'tab.next': cycleTab(1); break;
      case 'tab.previous': cycleTab(-1); break;
      case 'panel.left': views.set('sidebar', !views.current.sidebar); break;
      case 'panel.detail': views.set('details', !views.current.details); break;
      case 'help': showHelp = !showHelp; break;
      case 'palette': showPalette = !showPalette; break;
      case 'terminal': terminal.toggle(); break;
      case 'repo.open': void openAnother(); break;
      default: break;
    }
  }

  function cycleTab(delta: number) {
    const list = tabs.session.tabs;
    if (list.length === 0) return;
    const at = list.findIndex((t) => t.id === tabs.session.active);
    const next = list[(at + delta + list.length) % list.length];
    if (next) void tabs.activate(next.id);
  }
  let info = $state<RepoInfo | null>(null);
  let error = $state<string | null>(null);
  let scrollTop = $state(0);
  let viewport = $state(600);
  /** Width of the graph pane, which is what decides whether a body preview has room. */
  let paneWidth = $state(0);
  const panes = new PanesState();
  const diff = new DiffState(views);
  const actions = new ActionsState();
  const merge = new MergeState();
  const hosting = new HostingState();
  const rebase = new RebaseState();
  const signing = new SigningState();
  const ssh = new SshState();
  const toasts = new ToastsState();
  const remotes = new RemotesState();
  let showRemotes = $state<{ focus: string | null } | null>(null);
  /** The submodule whose panel is open, and its recorded commit once that has been read. */
  let showSubmodule = $state<Submodule | null>(null);
  let submoduleAt = $state<SubmoduleRevision | null>(null);
  /** The context menu on screen, if any. */
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  let showPrefs = $state(false);
  const terminal = new TerminalState(views);
  let showPalette = $state(false);

  /**
   * The question on screen, if any.
   *
   * One at a time and answered through a callback, so the flow that asked it reads top to
   * bottom instead of being split across a component's events.
   */
  let question = $state<{
    title: string;
    detail: string;
    placeholder: string;
    initial: string;
    choices: Choice[];
    answer: (choice: string | null, text: string) => void;
  } | null>(null);

  function ask(
    q: Omit<typeof question & object, 'answer'>,
  ): Promise<{ choice: string | null; text: string }> {
    return new Promise((resolve) => {
      question = {
        ...q,
        answer: (choice, text) => {
          question = null;
          resolve({ choice, text });
        },
      };
    });
  }
  /**
   * Asks for one line of text, or null when the user cancelled.
   *
   * Wraps `ask` because most callers want a string and nothing else, and repeating the choice
   * plumbing at every call site is what makes a dialog inconsistent.
   */
  async function askText(title: string, detail: string, initial: string): Promise<string | null> {
    const { choice, text } = await ask({
      title,
      detail,
      placeholder: '',
      initial,
      choices: [{ id: 'ok', label: 'OK', primary: true }],
    });
    return choice === null ? null : text;
  }

  let scroller = $state<HTMLDivElement | null>(null);

  const headName = $derived(
    info && info.head.kind !== 'detached' ? info.head.name : null,
  );

  function pick(row: number) {
    const local = localRow(graph.frame, row);
    if (local === null || !graph.frame || !info) return;
    showWip = false;
    void selection.select(info.path, row, oidOf(graph.frame, local));
  }

  /** Reloads everything after an operation finished, since it may have moved any of it. */
  async function reloadAll() {
    if (!info) return;
    const path = info.path;
    await Promise.all([refs.load(path), worktree.load(path), merge.load(path)]);
    await graph.open(path);
  }

  /** Every ref and where it points, as one string, to tell whether an action moved anything. */
  function refSignature(): string {
    return refs.all.map((r) => `${r.name}@${r.target}`).join('\u0000');
  }

  /**
   * Runs an action and reloads whatever it could have changed.
   *
   * Refs and status always, the graph only when a ref actually moved: rewalking 1.4M commits
   * after a stash that touched no ref would freeze the window for five seconds for nothing.
   */
  async function act(action: Action) {
    if (!info) return;
    const path = info.path;
    const before = refSignature();
    const outcome = await actions.run(path, action);
    if (!outcome) {
      // `actions` keeps the message for the status line; the toast is what carries it to
      // someone who is not looking at the bottom of the window.
      if (actions.report) toasts.push('error', 'Something went wrong', actions.report.text);
      return;
    }

    const said = describe(outcome.what, outcome.message, outcome.conflicted);
    toasts.push(said.kind, said.title, said.detail);

    await Promise.all([refs.load(path), worktree.load(path), merge.load(path)]);
    const after = refSignature();
    if (before !== after) await graph.open(path);

    // A checkout moves HEAD, and leaving the view where it was is the commonest way to end up
    // reading the branch that was just left.
    if (action.kind === 'checkout') {
      info = await open(path).catch(() => info);
      await focusHead();
    }
  }

  /**
   * What right-clicking a commit offers.
   *
   * Grouped as the reference groups them: where to go, what to make here, how to rewrite the
   * history, what to copy, and what to tag. The history edits are refused by the engine for a
   * commit that is not on this branch or for a range holding a merge, so nothing here has to
   * guess at whether they are safe.
   */
  /** The row under the pointer, if the loaded frame reaches it. */
  function rightClickRow(event: MouseEvent, row: number) {
    const local = localRow(graph.frame, row);
    if (local === null || !graph.frame) return;
    commitMenu(event, row, oidOf(graph.frame, local));
  }

  function commitMenu(event: MouseEvent, row: number, oid: string) {
    event.preventDefault();
    pick(row);
    const short = oid.slice(0, 8);
    const branch = headName ?? 'HEAD';
    const summary = graph.meta.get(row)?.summary ?? '';

    menu = {
      x: event.clientX,
      y: event.clientY,
      items: [
        {
          kind: 'item',
          label: 'Checkout this commit',
          hint: short,
          run: () => void act({ kind: 'checkout', rev: oid }),
        },
        { kind: 'item', label: 'Create worktree from this commit', run: () => void worktreeAt(oid) },
        { kind: 'separator' },
        { kind: 'item', label: 'Create branch here', run: () => void branchAt(oid) },
        {
          kind: 'item',
          label: 'Cherry pick commit',
          run: () => void act({ kind: 'cherryPick', revs: [oid] }),
        },
        {
          kind: 'submenu',
          label: `Reset ${branch} to this commit`,
          items: [
            {
              kind: 'item',
              label: 'Soft — keep the index and the working copy',
              run: () => void act({ kind: 'reset', rev: oid, mode: 'soft' }),
            },
            {
              kind: 'item',
              label: 'Mixed — keep the working copy',
              run: () => void act({ kind: 'reset', rev: oid, mode: 'mixed' }),
            },
            {
              kind: 'item',
              label: 'Hard — discard everything since',
              danger: true,
              run: () => void confirmHardReset(oid, branch),
            },
          ],
        },
        {
          kind: 'item',
          label: 'Revert commit',
          run: () => void act({ kind: 'revert', revs: [oid] }),
        },
        { kind: 'separator' },
        {
          kind: 'item',
          label: `Interactive rebase the children of ${short}`,
          run: () => info && void rebase.load(info.path, `${oid}~1`),
        },
        { kind: 'item', label: 'Edit commit message', run: () => void reword(oid, summary) },
        {
          kind: 'item',
          label: 'Drop commit',
          danger: true,
          run: () => void confirmDrop(oid, summary),
        },
        {
          kind: 'item',
          label: 'Move commit up',
          run: () => void act({ kind: 'rewrite', rev: oid, how: 'moveNewer', message: null }),
        },
        {
          kind: 'item',
          label: 'Move commit down',
          run: () => void act({ kind: 'rewrite', rev: oid, how: 'moveOlder', message: null }),
        },
        { kind: 'separator' },
        { kind: 'item', label: 'Copy commit sha', hint: short, run: () => void copySha(oid) },
        { kind: 'item', label: 'Copy link to this commit', run: () => void copyLink(oid) },
        { kind: 'item', label: 'Create patch from commit', run: () => void patchOf(oid) },
        { kind: 'separator' },
        { kind: 'item', label: 'Create tag here', run: () => void tagAt(oid, false) },
        { kind: 'item', label: 'Create annotated tag here', run: () => void tagAt(oid, true) },
      ],
    };
  }

  async function branchAt(oid: string) {
    const name = await askText('Create branch here', `At ${oid.slice(0, 8)}.`, '');
    if (name === null || name.trim() === '') return;
    await act({ kind: 'branchCreate', name: name.trim(), at: oid, checkout: true });
  }

  async function tagAt(oid: string, annotated: boolean) {
    const name = await askText(
      annotated ? 'Create annotated tag here' : 'Create tag here',
      `At ${oid.slice(0, 8)}.`,
      '',
    );
    if (name === null || name.trim() === '') return;
    let message: string | null = null;
    if (annotated) {
      message = await askText('Tag message', `For ${name.trim()}.`, '');
      if (message === null) return;
    }
    await act({ kind: 'tagCreate', name: name.trim(), at: oid, message });
  }

  async function worktreeAt(oid: string) {
    const where = await pickDirectory('Where should the new working tree go?');
    if (where === null) return;
    const branch = await askText(
      'Branch for the new working tree',
      'Leave it empty to check the commit out detached. Two working trees may not share a branch.',
      '',
    );
    if (branch === null) return;
    await act({
      kind: 'worktreeAdd',
      path: where,
      rev: oid,
      branch: branch.trim() === '' ? null : branch.trim(),
    });
  }

  async function patchOf(oid: string) {
    const where = await pickDirectory('Where should the patch be written?');
    if (where === null) return;
    await act({ kind: 'patch', rev: oid, directory: where });
  }

  async function reword(oid: string, summary: string) {
    const message = await askText(
      'Edit commit message',
      'Everything above this commit is replayed, so their object ids change.',
      summary,
    );
    if (message === null || message.trim() === '') return;
    await act({ kind: 'rewrite', rev: oid, how: 'reword', message: message.trim() });
  }

  async function confirmDrop(oid: string, summary: string) {
    const { choice } = await ask({
      title: 'Drop this commit?',
      detail: `${summary}\n\nIt is removed and everything above it is replayed, so their object ids change.`,
      placeholder: '',
      initial: '',
      choices: [{ id: 'drop', label: 'Drop the commit', primary: true }],
    });
    if (choice === null) return;
    await act({ kind: 'rewrite', rev: oid, how: 'drop', message: null });
  }

  async function confirmHardReset(oid: string, branch: string) {
    const { choice } = await ask({
      title: `Reset ${branch} hard?`,
      detail:
        'Uncommitted changes in the working copy are discarded and cannot be recovered. The ' +
        'commits themselves stay in the reflog.',
      placeholder: '',
      initial: '',
      choices: [{ id: 'reset', label: 'Discard and reset', primary: true }],
    });
    if (choice === null) return;
    await act({ kind: 'reset', rev: oid, mode: 'hard' });
  }

  async function copySha(oid: string) {
    if (await copyText(oid)) toasts.push('ok', 'Copied', oid);
    else toasts.push('error', 'Could not reach the clipboard');
  }

  async function copyLink(oid: string) {
    if (!info) return;
    const url = await commitUrl(info.path, oid, null).catch(() => null);
    if (url === null) {
      toasts.push('info', 'No web address for this commit', 'The remote is not a host Coral knows.');
      return;
    }
    if (await copyText(url)) toasts.push('ok', 'Copied', url);
    else toasts.push('error', 'Could not reach the clipboard');
  }

  /**
   * The refs a row had no room for.
   *
   * The overflow chip has to lead somewhere: a row can carry a dozen tags, and a count that
   * cannot be opened only says how many are being hidden from you.
   */
  function refsMenu(event: MouseEvent, hidden: PlacedRef[]) {
    event.stopPropagation();
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    menu = {
      x: box.left,
      y: box.bottom + 2,
      items: hidden.map((r) => ({
        kind: 'submenu' as const,
        label: r.short,
        items: [
          {
            kind: 'item' as const,
            label: 'Go to it',
            disabled: r.row === null,
            run: () => r.row !== null && void reveal(r.row),
          },
          {
            kind: 'item' as const,
            label: `Checkout ${r.short}`,
            run: () => void act({ kind: 'checkout', rev: r.short }),
          },
        ],
      })),
    };
  }

  function openPreferences() {
    showPrefs = true;
    if (!info) return;
    void signing.load(info.path);
    void ssh.load(info.path);
  }

  /** How a pull should integrate, offered at the caret beside the Pull button. */
  function pullMenu(event: MouseEvent) {
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    menu = {
      x: box.left,
      y: box.bottom + 2,
      items: [
        {
          kind: 'item',
          label: 'Pull, fast-forward only',
          hint: 'refuses to merge',
          run: () => void act({ kind: 'pull', remote: null, mode: 'ffOnly' }),
        },
        {
          kind: 'item',
          label: 'Pull, merging',
          run: () => void act({ kind: 'pull', remote: null, mode: 'merge' }),
        },
        {
          kind: 'item',
          label: 'Pull, rebasing',
          run: () => void act({ kind: 'pull', remote: null, mode: 'rebase' }),
        },
        { kind: 'separator' },
        {
          kind: 'item',
          label: 'Fetch every remote',
          run: () => void act({ kind: 'fetch', remote: null }),
        },
      ],
    };
  }

  /** What right-clicking a remote, or the Remote section itself, offers. */
  function remoteMenu(event: MouseEvent, remote: string | null) {
    event.preventDefault();
    const items: MenuItem[] = [
      { kind: 'item', label: 'Add a remote…', run: () => (showRemotes = { focus: null }) },
    ];
    if (remote !== null) {
      const found = remotes.list.find((r) => r.name === remote);
      items.unshift(
        {
          kind: 'item',
          label: `Remote details: ${remote}`,
          hint: found?.fetchUrl ? '' : undefined,
          run: () => (showRemotes = { focus: remote }),
        },
        { kind: 'item', label: 'Edit URL or rename…', run: () => (showRemotes = { focus: remote }) },
        { kind: 'separator' },
        { kind: 'item', label: `Fetch ${remote}`, run: () => void act({ kind: 'fetch', remote }) },
        {
          kind: 'item',
          label: 'Prune branches that are gone',
          run: () => void pruneRemote(remote),
        },
        { kind: 'separator' },
        {
          kind: 'item',
          label: `Copy ${remote}'s URL`,
          disabled: !found,
          run: () => void copyRemoteUrl(remote),
        },
        {
          kind: 'item',
          label: `Remove ${remote}`,
          danger: true,
          run: () => void confirmRemoveRemote(remote),
        },
        { kind: 'separator' },
      );
    }
    menu = { x: event.clientX, y: event.clientY, items };
  }

  async function pruneRemote(remote: string) {
    if (await remotes.edit({ kind: 'prune', name: remote })) {
      toasts.push('ok', `Pruned ${remote}`);
      if (info) await refs.load(info.path);
    } else {
      toasts.push('error', `Could not prune ${remote}`, remotes.error ?? '');
    }
  }

  async function copyRemoteUrl(remote: string) {
    const url = remotes.list.find((r) => r.name === remote)?.fetchUrl ?? '';
    if (url && (await copyText(url))) toasts.push('ok', 'Copied', url);
    else toasts.push('error', 'Could not reach the clipboard');
  }

  async function confirmRemoveRemote(remote: string) {
    const { choice } = await ask({
      title: `Remove the remote ${remote}?`,
      detail: 'Its tracking branches go with it. Nothing on the server is touched.',
      placeholder: '',
      initial: '',
      choices: [{ id: 'remove', label: 'Remove it', primary: true }],
    });
    if (choice === null) return;
    if (await remotes.edit({ kind: 'remove', name: remote })) {
      toasts.push('ok', `Removed ${remote}`);
      if (info) await refs.load(info.path);
    } else {
      toasts.push('error', `Could not remove ${remote}`, remotes.error ?? '');
    }
  }

  /** Opens a working-tree file's diff, from whichever half of the panel it was clicked in. */
  function openWorkingFile(file: string, staged: boolean) {
    if (!info) return;
    void diff.openWorking(info.path, staged, file);
  }

  /** Opens one of the selected commit's files in the diff viewer. */
  function openFile(file: string) {
    const rev = selection.detail?.commit.oid;
    if (!info || rev === undefined) return;
    void diff.open(info.path, rev, file);
  }

  /**
   * Everything the palette offers.
   *
   * Built from the repository rather than a fixed list, so a branch can be checked out, merged
   * or rebased onto by name without a submenu for each.
   */
  const commands = $derived.by<Command[]>(() => {
    const out: Command[] = [
      { id: 'fetch', label: 'Fetch', group: 'Remote', run: () => void act({ kind: 'fetch', remote: null }) },
      { id: 'pull', label: 'Pull (fast-forward only)', group: 'Remote', run: () => void act({ kind: 'pull', remote: null, mode: 'ffOnly' }) },
      { id: 'pull-rebase', label: 'Pull, rebasing', group: 'Remote', run: () => void act({ kind: 'pull', remote: null, mode: 'rebase' }) },
      { id: 'push', label: 'Push', group: 'Remote', run: () => void act({ kind: 'push', remote: null, setUpstream: true }) },
      { id: 'stash', label: 'Stash changes', group: 'Stash', run: () => void act({ kind: 'stashPush', message: null }) },
      { id: 'pop', label: 'Pop the latest stash', group: 'Stash', run: () => void act({ kind: 'stashApply', index: 0, pop: true }) },
      { id: 'undo', label: 'Undo', group: 'History', run: () => void act({ kind: 'undo' }) },
      { id: 'redo', label: 'Redo', group: 'History', run: () => void act({ kind: 'redo' }) },
      { id: 'theme', label: 'Toggle dark mode', group: 'View', run: () => theme.toggle() },
      {
        id: 'terminal',
        label: terminal.open ? 'Hide the terminal' : 'Show the terminal',
        group: 'View',
        run: () => terminal.toggle(),
      },
      {
        id: 'terminal-dock',
        label: `Move the terminal to the ${terminal.dock === 'bottom' ? 'right' : 'bottom'}`,
        group: 'View',
        run: () => terminal.setDock(terminal.dock === 'bottom' ? 'right' : 'bottom'),
      },
      {
        id: 'signing',
        label: 'SSH keys and commit signing',
        group: 'View',
        run: openPreferences,
      },
    ];

    for (const r of refs.groups.local) {
      if (r.short === headName) continue;
      out.push({ id: `co:${r.name}`, label: `Checkout ${r.short}`, group: 'Branch', run: () => void act({ kind: 'checkout', rev: r.short }) });
      out.push({ id: `merge:${r.name}`, label: `Merge ${r.short} into ${headName ?? 'HEAD'}`, group: 'Branch', run: () => void act({ kind: 'merge', rev: r.short }) });
      out.push({ id: `rebase:${r.name}`, label: `Rebase onto ${r.short}`, group: 'Branch', run: () => void act({ kind: 'rebase', onto: r.short }) });
      out.push({ id: `irebase:${r.name}`, label: `Rebase onto ${r.short}, interactively`, group: 'Branch', run: () => info && void rebase.load(info.path, r.short) });
    }
    for (const r of refs.groups.tags.slice(0, 200)) {
      out.push({ id: `co:${r.name}`, label: `Checkout tag ${r.short}`, group: 'Tag', run: () => void act({ kind: 'checkout', rev: r.short }) });
    }
    return out;
  });

  /**
   * A branch dropped onto another.
   *
   * The reference reads the gesture as "bring `source` into `target`", which needs `target`
   * checked out first — dropping onto a branch you are not on otherwise merges into the wrong
   * one silently. Dropping a local branch onto its remote counterpart pushes instead, which is
   * the one case where the gesture means something else entirely.
   */
  async function dropRef(source: string, target: string) {
    const pushing = target === `origin/${source}` || target.endsWith(`/${source}`);
    if (pushing) {
      await act({ kind: 'push', remote: null, setUpstream: true });
      return;
    }
    const { choice } = await ask({
      title: `Bring ${source} into ${target}?`,
      detail:
        target === headName
          ? ''
          : `${target} will be checked out first, since that is the branch the work lands on.`,
      placeholder: '',
      initial: '',
      choices: [
        { id: 'merge', label: 'Merge', primary: true },
        { id: 'rebase', label: 'Rebase' },
      ],
    });
    if (choice === null) return;
    if (target !== headName) await act({ kind: 'checkout', rev: target });
    if (choice === 'rebase') await act({ kind: 'rebase', onto: source });
    else await act({ kind: 'merge', rev: source });
  }

  /** The toolbar's seven buttons, each the commonest form of its action. */
  function toolbarAction(name: string) {
    const branch = headName;
    switch (name) {
      case 'undo': return void act({ kind: 'undo' });
      case 'redo': return void act({ kind: 'redo' });
      case 'fetch': return void act({ kind: 'fetch', remote: null });
      case 'pull': return void act({ kind: 'pull', remote: null, mode: 'ffOnly' });
      // set-upstream on every push: it is a no-op once one is configured, and without it the
      // first push of a new branch fails with advice instead of pushing.
      case 'push': return void act({ kind: 'push', remote: null, setUpstream: true });
      case 'stash': return void act({ kind: 'stashPush', message: null });
      case 'pop': return void act({ kind: 'stashApply', index: 0, pop: true });
      case 'terminal': return terminal.toggle();
      case 'branch': {
        void (async () => {
          const { choice, text } = await ask({
            title: 'New branch',
            detail: `Created at ${branch ?? 'HEAD'} and checked out.`,
            placeholder: 'feature/…',
            initial: '',
            choices: [{ id: 'create', label: 'Create branch', primary: true }],
          });
          if (choice !== null && text !== '') {
            await act({ kind: 'branchCreate', name: text, at: null, checkout: true });
          }
        })();
        return;
      }
      default:
        return;
    }
  }

  function pickWip() {
    showWip = true;
    selection.clear();
    if (info) void worktree.load(info.path);
  }

  /**
   * What the WIP row summarises: how many files are waiting, by kind.
   *
   * Split into edits and additions rather than one total, as the reference does — nine new
   * files and nine changed ones are very different amounts of work to review.
   */
  const wip = $derived.by(() => {
    let edits = 0;
    let adds = 0;
    for (const entry of worktree.status?.entries ?? []) {
      const change = entry.index !== 'unmodified' ? entry.index : entry.worktree;
      if (change === 'added' || change === 'untracked') adds += 1;
      else edits += 1;
    }
    return { edits, adds };
  });

  /** Scrolls a row into view, used when a ref is picked in the sidebar. */
  /**
   * Scrolls a row into view and selects it.
   *
   * Following a branch in the sidebar should land on that commit, not merely somewhere near
   * it: without the selection the detail panel still describes whatever was picked last, and
   * nothing on the row that was scrolled to says it is the one that was asked for.
   */
  async function reveal(row: number) {
    scrollToRow(row);
    // A ref can point anywhere in the history, which is very unlikely to be inside whatever
    // frame is loaded, so the rows have to arrive before there is anything to select.
    await graph.ensureRows(row, row);
    pick(row);
  }

  /**
   * Scrolls the commit list, rather than leaving it to the browser.
   *
   * Two reasons, and the second is why it is here at all. Above `MAX_SPACER_PX` the scrollable
   * area no longer stands for the row range one pixel per row, so a wheel notch of 100px covers
   * a different number of commits depending on how large the repository is; converting the
   * delta to rows first makes a notch three commits everywhere. And the platform's own wheel
   * handling has proved unreliable in this webview, which leaves a list that can only be moved
   * by dragging a scrollbar thumb five pixels tall.
   *
   * `deltaMode` is honoured because a mouse reports lines and a trackpad reports pixels; taking
   * `deltaY` as pixels either way makes a mouse scroll three pixels a notch.
   */
  function wheel(event: WheelEvent) {
    if (!scroller) return;
    if (event.ctrlKey) return;

    const lines = event.deltaMode === 1 ? event.deltaY : event.deltaY / 40;
    const pages = event.deltaMode === 2 ? event.deltaY : 0;
    const rows =
      pages * Math.max(1, Math.floor(viewport / DEFAULT_METRICS.rowHeight) - 1) + lines * 3;
    if (rows === 0) return;

    const total = graph.totalRows;
    const height = spacerHeight(total, DEFAULT_METRICS);
    const reach = Math.max(1, height - viewport);
    // Below the cap a row is a whole pixel; above it the scrollable area is compressed, so the
    // same number of rows is a smaller number of pixels.
    const perRow = total > 0 ? reach / Math.max(1, total - 1) : DEFAULT_METRICS.rowHeight;

    const next = Math.max(0, Math.min(reach, scroller.scrollTop + rows * perRow));
    if (next !== scroller.scrollTop) {
      scroller.scrollTop = next;
      scrollTop = next;
    }
    event.preventDefault();
  }

  function scrollToRow(row: number) {
    if (!graph.frame || !scroller) return;
    // Above the height cap a row is a fraction of a pixel, so the target is the fraction of
    // the scrollable range rather than the row's pixel offset.
    const total = graph.totalRows;
    const height = spacerHeight(total, DEFAULT_METRICS);
    const lastTop = Math.max(1, total - Math.floor(viewport / DEFAULT_METRICS.rowHeight));
    const fraction = Math.max(0, row - 3) / lastTop;
    scroller.scrollTo({ top: Math.min(height - viewport, fraction * (height - viewport)) });
  }

  async function load(path: string) {
    error = null;
    forgetTheLastRepository();
    try {
      info = await open(path);
      await graph.open(info.path);
      await refs.load(info.path);
      await worktree.load(info.path);
      // A repository can be opened mid-merge, so the tool has to be there on arrival rather
      // than only after an action of ours stopped.
      await merge.load(info.path);
      void remotes.load(info.path);
      // Deliberately not awaited: a host that is slow or unreachable must not hold up the
      // window, and the section simply appears when the answer arrives.
      void hosting.load(info.path);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  /**
   * Drops everything that meant something only in the repository being left.
   *
   * A commit, the diff opened from it, a half-composed rebase, a scroll position: none of them
   * name anything in the next repository, and leaving them up shows one repository's contents
   * under another's name. The engine state is reloaded per repository anyway; this is the view
   * state that has no owner to reload it.
   */
  function forgetTheLastRepository() {
    selection.clear();
    diff.close();
    rebase.close();
    merge.close();
    actions.clear();
    menu = null;
    showRemotes = null;
    showSubmodule = null;
    submoduleAt = null;
    showWip = false;
    scrollTop = 0;
    if (scroller) scroller.scrollTop = 0;
  }

  /**
   * Restores the session, then opens whatever it was left on.
   *
   * CORAL_REPO still wins when set, so the app can be pointed at a repository for
   * benchmarking without disturbing the saved tabs.
   */
  async function start() {
    await tabs.refresh();
    const requested = await initialRepo();
    if (requested !== '.') await tabs.open(requested);
    else if (!tabs.active && tabs.session.tabs.length === 0) await tabs.open(requested);
    // The effect below loads whatever ends up active; loading here as well would walk the
    // graph twice on launch, which on a large repository is two five-second walks.
  }
  void start();

  // Switching tabs loads that repository; nothing else in the shell needs to know.
  let loadedPath = $state('');
  $effect(() => {
    // What the tab is showing, which is the submodule when one is open inside it.
    const path = tabs.workingPath;
    if (path && path !== loadedPath) {
      loadedPath = path;
      void load(path);
    }
  });

  /**
   * Moves the selection to whatever HEAD now points at.
   *
   * Checking a branch out and leaving the view where it was is the commonest way to end up
   * reading the wrong branch's commits: the row that is selected still belongs to the branch
   * that was just left. This follows the checkout, whether it was made here or in the terminal.
   */
  async function focusHead() {
    if (!info) return;
    const name = info.head.kind !== 'detached' ? info.head.name : null;
    const target = name === null
      ? refs.all.find((r) => r.kind.kind === 'local_branch' && r.row !== null)
      : refs.groups.local.find((r) => r.short === name);
    if (target?.row === undefined || target.row === null) return;
    await reveal(target.row);
  }

  /**
   * Reacts to the repository changing on disk.
   *
   * Reloads only what the change touched: rewalking 1.4M commits because a build wrote an
   * object file would freeze the window for five seconds, repeatedly. A ref change is the one
   * that can move HEAD, so it is the one that also moves the selection.
   */
  async function repoChanged(change: RepoChanged) {
    if (!info || actions.busy) return;
    const path = info.path;
    const wasHead = headName;

    if (change.index || change.worktree) await worktree.load(path);
    if (change.ops) await merge.load(path);
    if (!change.refs && !change.graph) return;

    info = await open(path).catch(() => info);
    await refs.load(path);
    if (change.refs || change.graph) await graph.open(path);
    // Only when it actually moved: following HEAD on every commit would drag the view away
    // from whatever the user was reading.
    if (headName !== wasHead) await focusHead();
  }

  /**
   * True when the worktree could not be watched, so file changes will not arrive on their own.
   *
   * On Linux inotify has a per-user watch limit that a tree the size of the kernel exhausts.
   * The engine reports that rather than going quietly stale, and the window has to do something
   * with the answer: say so once, and refresh when the window is looked at again.
   */
  let watchIsPartial = $state(false);

  // One watch, following whichever repository the active tab is showing.
  $effect(() => {
    const path = loadedPath;
    if (!path) return;
    void watchRepo(path)
      .then((watching) => {
        watchIsPartial = !watching.complete;
        if (watching.complete) return;
        toasts.push(
          'info',
          'Watching this working tree only partly',
          `${watching.detail ?? 'The system refused to watch it.'}\nCoral will refresh when ` +
            'you come back to the window.',
        );
      })
      .catch(() => {
        // Nothing is watching, so the same fallback applies.
        watchIsPartial = true;
      });
    return () => void unwatchRepo().catch(() => undefined);
  });

  /**
   * Refreshes what a file change would have told us, when nothing is telling us.
   *
   * Status and refs only. Rewalking the graph every time the window is focused would freeze it
   * for seconds on a large repository, and a commit made elsewhere arrives with the next
   * action anyway.
   */
  async function refreshOnFocus() {
    if (!watchIsPartial || !info || actions.busy) return;
    const path = info.path;
    await Promise.all([worktree.load(path), refs.load(path)]);
  }

  $effect(() => {
    const stop = onRepoChanged((change) => void repoChanged(change));
    return () => void stop.then((off) => off());
  });

  async function openAnother() {
    const path = await pickRepository();
    if (path) await tabs.open(path);
  }

  /**
   * Shows a submodule inside the tab that declares it.
   *
   * Not a tab of its own. A submodule belongs to the repository that declares it, at the commit
   * that repository records; opening a second tab loses that relationship and leaves two
   * entries in the bar with no way to tell which came from which. The reference shows it as
   * another step in the same tab's breadcrumb, and so does this.
   */
  async function openSubmodule(relative: string) {
    const tab = tabs.active;
    if (!tab) return;
    // One that has never been checked out has nothing to open; the menu offers to fetch it.
    const known = refs.submodules.find((s) => s.path === relative);
    if (known && !known.initialised) {
      await initSubmodule(relative, false);
      return;
    }
    if (tab.submodule === relative) await tabs.leaveSubmodule(tab.id);
    else await tabs.enterSubmodule(tab.id, relative);
  }

  /**
   * Updates one submodule, or all of them when `relative` is null.
   *
   * `remote` is a different operation, not a variation: without it the working copy moves to
   * the commit the superproject records, with it git fetches the configured branch and moves
   * to its tip — which changes what the superproject will record next.
   */
  async function initSubmodule(relative: string | null, remote: boolean) {
    if (!info) return;
    const what = relative ?? 'every submodule';
    const { choice } = await ask({
      title: remote ? `Update ${what} to its branch tip?` : `Update ${what}?`,
      detail: remote
        ? 'git fetches the configured branch and checks out its tip. That is a change the ' +
          'repository will record on the next commit.'
        : 'The working copy is cloned or moved to the commit the repository records. This ' +
          'reaches the network.',
      placeholder: '',
      initial: '',
      choices: [{ id: 'go', label: 'Update it', primary: true }],
    });
    if (choice === null) return;
    await act({ kind: 'submoduleInit', path: relative, recursive: false, remote });
    await refs.load(info.path);
    await refreshSubmodule();
  }

  /**
   * What the dots beside a submodule offer, and what a right-click on its row does.
   *
   * The row itself is not a control. Opening a submodule is one of four things that can be
   * done with it, and making it the one a click performs leaves the other three hidden.
   */
  function submoduleMenu(event: MouseEvent, submodule: Submodule) {
    event.preventDefault();
    event.stopPropagation();
    const at = event.currentTarget instanceof HTMLElement
      ? event.currentTarget.getBoundingClientRect()
      : null;
    menu = {
      x: at && event.type === 'click' ? at.right : event.clientX,
      y: at && event.type === 'click' ? at.bottom + 2 : event.clientY,
      items: [
        {
          kind: 'item',
          label: 'Edit this submodule…',
          run: () => void openSubmodulePanel(submodule),
        },
        {
          kind: 'item',
          label: 'Open this submodule',
          disabled: !submodule.initialised,
          run: () => void openSubmodule(submodule.path),
        },
        {
          kind: 'submenu',
          label: 'Update this submodule',
          items: [
            {
              kind: 'item',
              label: submodule.initialised
                ? 'To the commit this repository records'
                : 'Fetch a working copy',
              run: () => void initSubmodule(submodule.path, false),
            },
            {
              kind: 'item',
              label: 'To the tip of its branch',
              run: () => void initSubmodule(submodule.path, true),
            },
          ],
        },
        { kind: 'separator' },
        {
          kind: 'item',
          label: 'Copy its URL',
          disabled: submodule.url === '',
          run: () => void copyUrl(submodule.url),
        },
        {
          kind: 'item',
          label: 'Delete this submodule',
          danger: true,
          run: () => {
            showSubmodule = submodule;
            void removeSubmodule();
          },
        },
      ],
    };
  }

  async function copyUrl(url: string) {
    if (await copyText(url)) toasts.push('ok', 'Copied', url);
    else toasts.push('error', 'Could not reach the clipboard');
  }

  /** Opens the submodule panel and reads the commit it is pinned at. */
  async function openSubmodulePanel(submodule: Submodule) {
    showSubmodule = submodule;
    submoduleAt = null;
    await refreshSubmodule();
  }

  async function refreshSubmodule() {
    const at = showSubmodule?.path;
    if (!info || at === undefined) return;
    submoduleAt = await submoduleRevision(info.path, at).catch(() => null);
  }

  async function setSubmoduleUrl(url: string) {
    const at = showSubmodule?.path;
    if (!info || at === undefined) return;
    await act({ kind: 'submoduleSetUrl', path: at, url });
    await refs.load(info.path);
    showSubmodule = refs.submodules.find((s) => s.path === at) ?? showSubmodule;
  }

  async function removeSubmodule() {
    const submodule = showSubmodule;
    if (!info || !submodule) return;
    const { choice } = await ask({
      title: `Delete the submodule ${submodule.path}?`,
      detail:
        'Its working copy, its entry in .gitmodules and its clone under .git/modules all go. ' +
        'Nothing on the server is touched, and the deletion is staged rather than committed.',
      placeholder: '',
      initial: '',
      choices: [{ id: 'go', label: 'Delete it', primary: true }],
    });
    if (choice === null) return;
    // Forced: a submodule with local edits refuses otherwise, and the user has just been told
    // exactly what is being removed.
    await act({ kind: 'submoduleRemove', path: submodule.path, force: true });
    showSubmodule = null;
    submoduleAt = null;
    await Promise.all([refs.load(info.path), worktree.load(info.path)]);
  }

  async function leaveSubmodule() {
    const tab = tabs.active;
    if (tab) await tabs.leaveSubmodule(tab.id);
  }

  /**
   * Whether there is room for the dimmed body preview after the summary.
   *
   * Decided here rather than with a container query: `container-type` collapses the message
   * cell of the WIP row, which is a button, and the row loses its text entirely. Three
   * characters of a continuation is noise anyway — below this the summary takes the width.
   */
  const showBody = $derived(
    paneWidth - panes.widths.refs - panes.widths.graph > 560,
  );

  /** Rows currently worth putting in the DOM. Never the whole graph. */
  function windowRows(frame: Frame | null): number[] {
    if (!frame) return [];
    const perScreen = Math.ceil(viewport / DEFAULT_METRICS.rowHeight);
    const first = firstRowFor(scrollTop, viewport, frame.totalRows, DEFAULT_METRICS);
    const last = Math.min(frame.totalRows - 1, first + perScreen + 2);
    const out: number[] = [];
    for (let r = first; r <= last; r++) out.push(r);
    return out;
  }


  const rows = $derived(windowRows(graph.frame));

  /**
   * Reading `graph.meta` here rather than inside the canvas keeps the redraw reactive: the
   * identity of this function changes whenever a metadata block lands, which is the signal the
   * canvas repaints on.
   */
  const nodeInitials = $derived.by(() => {
    const meta = graph.meta;
    return (row: number) => {
      const author = meta.get(row)?.author;
      return author === undefined ? null : initialsOf(author);
    };
  });

  /**
   * What fixes a node's colour.
   *
   * The email, not the display name: the same person commits as "Linus Torvalds" and
   * "torvalds" over a long history, and a node that changes colour partway down the graph
   * defeats the point of colouring it.
   */
  const nodeAuthor = $derived.by(() => {
    const meta = graph.meta;
    return (row: number) => {
      const entry = meta.get(row);
      if (entry === undefined) return null;
      return entry.email.trim().toLowerCase() || entry.author;
    };
  });


  // Only rows that are on screen are worth an object read, or a frame.
  $effect(() => {
    if (rows.length === 0) return;
    const first = rows[0] ?? 0;
    const last = rows[rows.length - 1] ?? first;
    void graph.ensureRows(first, last);
    void graph.loadMetadata(first, rows.length);
  });

  /**
   * Which host a tracking branch's remote belongs to.
   *
   * From the remote's URL, not from its name: a remote called `origin` says nothing about who
   * serves it, and a repository can have one on each host.
   */
  function hostFor(short: string): 'github' | 'gitlab' | 'other' {
    const remote = short.split('/')[0] ?? '';
    const url = remotes.list.find((r) => r.name === remote)?.fetchUrl ?? '';
    return url === '' ? 'other' : hostOf(url);
  }

  /**
   * Which of a row's refs are worth the two slots there are.
   *
   * The checked-out branch first, then other local branches, then tags, then tracking branches:
   * a row can carry a dozen labels and the ones cut have to be the ones that say least. A
   * tracking branch beside the local branch it tracks is the commonest pair, and it is the
   * tracking one that repeats what is already there.
   */
  function orderRefs(labels: PlacedRef[]): PlacedRef[] {
    const rank = (r: PlacedRef): number => {
      if (r.short === headName) return 0;
      switch (r.kind.kind) {
        case 'local_branch':
          return 1;
        case 'tag':
          return 2;
        case 'stash':
          return 3;
        default:
          return 4;
      }
    };
    return [...labels].sort((a, b) => rank(a) - rank(b));
  }

  /**
   * The body as one dimmed line after the summary, as the reference shows it. Newlines become
   * a separator rather than being dropped, so a bullet list still reads as several points.
   */
  function flatten(body: string): string {
    return body
      .split('\n')
      .map((l) => l.trim())
      .filter((l) => l.length > 0)
      .join(' | ');
  }

  function when(seconds: number): string {
    const delta = Date.now() / 1000 - seconds;
    const hours = delta / 3600;
    if (hours < 24) return `${Math.max(1, Math.round(hours))}h`;
    const days = hours / 24;
    if (days < 365) return `${Math.round(days)}d`;
    return `${(days / 365).toFixed(1)}y`;
  }
</script>

<svelte:window onkeydown={onKey} onfocus={() => void refreshOnFocus()} />

<main>
  <header>
    <h1>Coral</h1>
    {#if info}
      <span class="path mono" title={info.path}>{elidePath(info.path, 64)}</span>
      {#if graph.provisional}
        <span class="chip warn" title="Commit-time order, being replaced by the topological walk">
          provisional order
        </span>
      {/if}
    {/if}
    <button
      class="theme"
      onclick={() => openPreferences()}
      title="SSH keys, signing and preferences"
      aria-label="Settings"
    >⚙</button>
    <button class="theme" onclick={() => theme.toggle()} title="Switch theme">
      {theme.current === 'light' ? 'Dark' : 'Light'}
    </button>
  </header>

  <TabBar {tabs} onOpen={openAnother} onAsk={askText} />

  {#if info}
    <Toolbar
      repo={TabsState.title(tabs.active ?? { id: 0, path: info.path, submodule: null, group: null, missing: false })}
      submodule={tabs.active?.submodule ?? null}
      branch={headName ?? 'detached'}
      busy={worktree.busy || actions.busy}
      terminalOpen={terminal.open}
      onAction={toolbarAction}
      onLeaveSubmodule={() => void leaveSubmodule()}
      onPullMenu={pullMenu}
    />
  {/if}

  {#if graph.transportWarning}
    <p class="banner">{graph.transportWarning}</p>
  {/if}
  {#if error}
    <p class="banner error">{error}</p>
  {:else if graph.error}
    <p class="banner error">{graph.error}</p>
  {/if}

  {#if graph.loading && !graph.frame}
    <Splash
      repo={loadedPath.split('/').filter(Boolean).at(-1) ?? loadedPath}
      path={loadedPath}
      stage={!info ? 'opening' : graph.provisional ? 'ordering' : 'walking'}
    />
  {:else if graph.frame}
    <div
      class="workspace"
      class:beside={terminal.open && terminal.dock === 'right'}
    >
    <div
      class="body"
      style:--refs-col="{panes.widths.refs}px"
      style:--graph-col="{panes.widths.graph}px"
      style:--sidebar-w="{panes.widths.sidebar}px"
      style:--details-w="{panes.widths.details}px"
    >
    {#if views.current.sidebar}
      <Sidebar
        groups={refs.groups}
        head={headName}
        submodules={refs.submodules}
        remotes={remotes.list}
        openSubmodule={tabs.active?.submodule ?? null}
        onSelect={reveal}
        onOpenSubmodule={openSubmodule}
        onRemoteMenu={remoteMenu}
        onInitAllSubmodules={() => void initSubmodule(null, false)}
        onSubmoduleMenu={submoduleMenu}
        onDropRef={dropRef}
        pullRequests={hosting.pullRequests}
        pullRequestLabel={hosting.view?.host?.kind === 'gitlab' ? 'Merge requests' : 'Pull requests'}
        collapsed={views.current.collapsed}
        onCollapse={(section, closed) => views.setCollapsed(section, closed)}
        onOpenPullRequest={(pr: PullRequest) => void openInBrowser(pr.webUrl)}
      />
      <Splitter
        label="Resize the sidebar"
        value={panes.widths.sidebar}
        min={PANE_LIMITS.sidebar.min}
        max={PANE_LIMITS.sidebar.max}
        onresize={(px) => panes.resize('sidebar', px)}
        onreset={() => panes.reset()}
      />
    {/if}
    {#if showPrefs}
      <Preferences
        {signing}
        {ssh}
        onClose={() => (showPrefs = false)}
        onCopied={(ok, what) =>
          ok
            ? toasts.push('ok', `Copied the ${what}`)
            : toasts.push('error', `Could not copy the ${what}`)}
      />
    {/if}
    {#if showSubmodule && info}
      <SubmodulePanel
        submodule={showSubmodule}
        revision={submoduleAt}
        busy={actions.busy}
        error={actions.report?.tone === 'error' ? actions.report.text : null}
        onClose={() => (showSubmodule = null)}
        onSetUrl={(url) => void setSubmoduleUrl(url)}
        onOpen={() => {
          const at = showSubmodule?.path;
          showSubmodule = null;
          if (at) void openSubmodule(at);
        }}
        onUpdate={(remote) => void initSubmodule(showSubmodule?.path ?? null, remote)}
        onRemove={() => void removeSubmodule()}
      />
    {/if}
    {#if showRemotes && info}
      <Remotes
        {remotes}
        focus={showRemotes.focus}
        onClose={() => (showRemotes = null)}
        onChanged={() => info && void refs.load(info.path)}
      />
    {/if}
    {#if merge.inProgress}
      <!-- A stopped merge or rebase is the only thing that matters until it is settled, so it
           takes the main pane outright rather than sitting behind the graph. -->
      <MergeTool {merge} onDone={reloadAll} />
    {:else if diff.path !== null}
      <DiffView {diff} onClose={() => diff.close()} />
    {/if}
    <div
      class="graph"
      class:hidden={diff.path !== null || merge.inProgress}
      bind:this={scroller}
      onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
      onwheel={wheel}
      bind:clientHeight={viewport}
      bind:clientWidth={paneWidth}
    >
      <div class="columns">
        <span class="col refs">
          Branch / Tag
          <Splitter
            label="Resize the branch and tag column"
            value={panes.widths.refs}
            min={PANE_LIMITS.refs.min}
            max={PANE_LIMITS.refs.max}
            onresize={(px) => panes.resize('refs', px)}
            onreset={() => panes.reset()}
          />
        </span>
        <span class="col graph-col">
          Graph
          <Splitter
            label="Resize the graph column"
            value={panes.widths.graph}
            min={PANE_LIMITS.graph.min}
            max={PANE_LIMITS.graph.max}
            onresize={(px) => panes.resize('graph', px)}
            onreset={() => panes.reset()}
          />
        </span>
        <span class="col message">Commit message</span>
      </div>
      {#if worktree.dirty}
        <button class="row wip" class:selected={showWip} onclick={pickWip}>
          <span class="cell refs"></span>
          <span class="cell graph-col"><span class="wip-node"></span></span>
          <span class="cell message">
            <span class="summary">WIP on {headName ?? 'HEAD'}</span>
            {#if wip.edits > 0}<span class="tally edit">✎ {wip.edits}</span>{/if}
            {#if wip.adds > 0}<span class="tally add">+ {wip.adds}</span>{/if}
          </span>
        </button>
      {/if}
      <div
        class="spacer"
        style:height="{spacerHeight(graph.totalRows, DEFAULT_METRICS)}px"
      >
        <div class="lanes" style:top="{listTop(scrollTop)}px">
          <GraphCanvas
            frame={graph.frame}
            firstRow={rows[0] ?? 0}
            height={viewport}
            width={panes.widths.graph}
            initials={nodeInitials}
            author={nodeAuthor}
          />
        </div>
        <ul class="rows" style:top="{listTop(scrollTop)}px">
          {#each rows as row (row)}
            <!--
              A row the loaded frame does not reach is left blank rather than read out of the
              wrong end of a typed array, which would show another commit's date and hash.
            -->
            {@const local = localRow(graph.frame, row)}
            {@const labels = orderRefs(refs.byRow.get(row) ?? [])}
            <li
              class="row"
              class:merge={hasFlag(graph.frame.rowFlags[local ?? -1] ?? 0, RowFlag.Merge)}
              class:selected={selection.row === row}
              oncontextmenu={(e) => rightClickRow(e, row)}
            >
              <button class="hit" onclick={() => pick(row)} aria-label="Select commit"></button>
              <!--
                Stacked, not side by side. Two pills abreast in a 190px column clip the second
                one, and a clipped ref name is every branch that starts the same way; stacked,
                each gets the column's whole width. Two is what the row's height allows.
              -->
              <span class="cell refs">
                {#each labels.slice(0, 2) as label, i (label.name)}
                  <span class="line">
                    <span
                      class="pill {label.kind.kind}"
                      class:head={label.short === headName}
                      title={label.name}
                    >
                      {#if label.kind.kind === 'remote_branch'}
                        <HostMark kind={hostFor(label.short)} />
                      {:else}
                        <span class="pip" aria-hidden="true"></span>
                      {/if}{elideRef(label.short, 22)}
                    </span>
                    {#if i === 1 && labels.length > 2}
                      <button
                        class="pill more"
                        title="Show the other refs on this commit"
                        onclick={(e) => refsMenu(e, labels.slice(2))}
                      >+{labels.length - 2}</button>
                    {/if}
                  </span>
                {/each}
              </span>
              <span class="cell graph-col"></span>
              <span class="cell message">
                <span class="summary">{graph.meta.get(row)?.summary ?? ''}</span>
                {#if showBody}
                  <span class="detail">{flatten(graph.meta.get(row)?.body ?? '')}</span>
                {/if}
                {#if local !== null}
                  <span class="age">{when(graph.frame.times[local] ?? 0)}</span>
                  <span class="sha mono">{oidOf(graph.frame, local).slice(0, 8)}</span>
                {/if}
              </span>
            </li>
          {/each}
        </ul>
      </div>
    </div>
    {#if views.current.details}
      <Splitter
        label="Resize the detail panel"
        value={panes.widths.details}
        min={PANE_LIMITS.details.min}
        max={PANE_LIMITS.details.max}
        grows="left"
        onresize={(px) => panes.resize('details', px)}
        onreset={() => panes.reset()}
      />
      {#if showWip}
        <aside class="wip-panel">
          <Staging
            {worktree}
            branch={headName}
            openPath={diff.path}
            grouping={views.current.changes}
            onGrouping={(g) => views.set('changes', g)}
            onOpenFile={openWorkingFile}
          />
        </aside>
      {:else}
        <Details
          detail={selection.detail}
          loading={selection.loading}
          error={selection.error}
          openPath={diff.path}
          grouping={views.current.commitFiles}
          onGrouping={(g) => views.set('commitFiles', g)}
          onOpenFile={openFile}
        />
      {/if}
    {/if}
    </div>

    {#if terminal.open && info}
      <Splitter
        label="Resize the terminal"
        value={terminal.size}
        min={120}
        max={900}
        grows={terminal.dock === 'right' ? 'left' : 'left'}
        vertical={terminal.dock === 'bottom'}
        onresize={(px) => terminal.setSize(px)}
        onreset={() => terminal.setDock(terminal.dock)}
      />
      <div
        class="term"
        style:height={terminal.dock === 'bottom' ? `${terminal.size}px` : undefined}
        style:width={terminal.dock === 'right' ? `${terminal.size}px` : undefined}
      >
        <Terminal session={terminal} path={info.path} onClose={() => (terminal.open = false)} />
      </div>
    {/if}
    </div>
  {/if}

  {#if info}
    <StatusBar
      branch={headName}
      head={refs.groups.local.find((r) => r.short === headName)}
      commits={graph.totalRows}
      changed={worktree.status?.entries.length ?? 0}
      gitVersion={info.gitVersion}
      host={hosting.view}
      report={actions.report}
      busy={actions.busy || worktree.busy || merge.busy}
      onDismiss={() => actions.clear()}
    />
  {/if}
</main>

{#if question}
  <Ask
    title={question.title}
    detail={question.detail}
    placeholder={question.placeholder}
    initial={question.initial}
    choices={question.choices}
    onAnswer={question.answer}
  />
{/if}

{#if rebase.open}
  <RebasePicker {rebase} onDone={reloadAll} />
{/if}

{#if showPalette}
  <Palette {commands} onClose={() => (showPalette = false)} />
{/if}

{#if showHelp}
  <Shortcuts live={LIVE} onClose={() => (showHelp = false)} />
{/if}

{#if menu}
  <Menu x={menu.x} y={menu.y} items={menu.items} onClose={() => (menu = null)} />
{/if}

<Toasts {toasts} />

<style>
  :root { --refs-col: 190px; --graph-col: 170px; }
  main { display: flex; flex-direction: column; height: 100%; }
  header {
    display: flex; align-items: center; gap: var(--space-3);
    height: 44px; box-sizing: border-box; padding: 0 var(--space-4);
    border-bottom: 1px solid var(--border); background: var(--bg-1);
  }
  h1 {
    font-size: 14px; font-weight: 700; margin: 0; color: var(--accent);
    letter-spacing: 0.01em;
  }
  /* Shortened in script, not by `direction: rtl`: see `elidePath` for why that trick draws
     `/home/x` as `home/x/`. */
  .path {
    flex: 1; min-width: 0; color: var(--fg-2); font-size: 12px;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .chip {
    font-size: 11px; padding: 1px var(--space-2); border-radius: 999px;
    background: var(--bg-2); color: var(--fg-1); flex: 0 0 auto;
  }
  .chip.warn { background: var(--warn-soft); color: var(--warn); }
  /* The window's own controls, which act on the application rather than on the repository.
     Only the first is pushed away from the path; the rest sit against it. */
  .theme {
    font: inherit; font-size: 11px; cursor: pointer;
    padding: 2px var(--space-2); border-radius: 3px;
    border: 1px solid var(--border); background: var(--bg-2); color: var(--fg-1);
  }
  .theme:first-of-type { margin-left: auto; }
  .theme:hover { background: var(--bg-3); color: var(--fg-0); }
  .banner { margin: 0; padding: var(--space-2) var(--space-4); background: var(--bg-2); color: var(--fg-1); font-size: 12px; }
  .banner.error { color: var(--danger); }
  .muted { color: var(--fg-2); }
  /*
   * A first screen with something to read on it. What is happening and roughly how long it
   * takes, because on a repository the size of the kernel this is several seconds of nothing.
   */
  .empty {
    flex: 1; display: flex; flex-direction: column; justify-content: center; align-items: center;
    gap: var(--space-2); padding: var(--space-5); text-align: center;
  }

  /* Positioned, so the preferences screen can cover the panes without covering the
     window's own chrome. */
  /* The workspace and the terminal. Side by side when it is docked right, stacked when it is
     docked at the bottom, which is the only difference between the two positions. */
  .workspace { display: flex; flex-direction: column; flex: 1; min-height: 0; min-width: 0; }
  .workspace.beside { flex-direction: row; }
  .term { flex: 0 0 auto; display: flex; min-height: 0; min-width: 0; }
  .term > :global(.terminal) { flex: 1; min-width: 0; }
  /* Positioned, so the preferences screen can cover the panes without covering the
     window's own chrome. */
  .body { display: flex; flex: 1; min-height: 0; position: relative; }
  .graph { flex: 1; overflow-y: auto; position: relative; background: var(--bg-0); }
  /* Hidden rather than unmounted: remounting would refetch the frame and lose the scroll
     position every time a file is opened and closed. */
  .graph.hidden { display: none; }

  /* Column headers, matching the row grid below so the two cannot drift apart. */
  .columns, .row, .wip {
    display: grid;
    grid-template-columns: var(--refs-col) var(--graph-col) 1fr;
    align-items: center;
  }
  .columns {
    position: sticky; top: 0; z-index: 2;
    height: 26px; padding: 0 var(--space-3);
    background: var(--bg-1); border-bottom: 1px solid var(--border);
    font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.07em;
    color: var(--fg-2);
  }
  .col { overflow: hidden; position: relative; display: flex; align-items: center; }
  /* The handle sits on the column's right edge and spans the header's full height. */
  .col :global(.splitter) {
    position: absolute; right: 0; top: 0; bottom: 0; margin: 0 -4px 0 0;
  }

  .spacer { position: relative; }
  /* The canvas tracks the scroll position rather than being as tall as the graph: a canvas
     millions of pixels high exhausts GPU texture memory. */
  /* Absolutely positioned at the same rounded offset the rows use, rather than sticky: one
     fewer composited layer in the scroller, and the lanes cannot drift half a pixel from the
     text they belong to. */
  .lanes {
    position: absolute; left: calc(var(--refs-col) + var(--space-3));
    height: 0; pointer-events: none;
  }
  /* The list is positioned once and the rows stack inside it in normal flow. Positioning each
     row individually put every one of them at its own computed offset; laying them out
     normally means only one element can be off, and it is snapped. */
  .rows { list-style: none; margin: 0; padding: 0; position: absolute; left: 0; right: 0; }

  .row, .wip {
    position: relative; height: var(--row-h);
    padding: 0 var(--space-3);
    font-size: 12px; color: var(--fg-1);
    border: 0; background: none; font-family: inherit; text-align: left;
  }
  .wip { position: sticky; top: 22px; z-index: 1; cursor: pointer; background: var(--bg-0); }
  .row:hover .cell.message, .wip:hover { background: var(--bg-1); }
  /*
   * The selected commit, tinted and given a bar down its leading edge. On a screen of rows
   * that all look alike a tint alone is easy to lose, and the bar survives a hover passing
   * over a neighbour.
   */
  .row.selected .cell.message, .wip.selected {
    background: var(--accent-soft);
    box-shadow: inset 2px 0 0 var(--accent-line);
  }
  .row.selected .cell.message .summary { color: var(--fg-0); font-weight: 600; }
  /*
   * The text columns paint an opaque background of their own. Over a transparent composited
   * layer WebKit drops from subpixel to grayscale antialiasing, which reads as soft — and
   * these rows sit above a canvas, which is what promotes the layer. The lane column stays
   * transparent so the canvas shows through it.
   */
  .cell.message, .cell.refs { background: var(--bg-0); }
  .cell.message {
    border-radius: var(--radius-1); padding: 0 var(--space-2);
    /* The row's own height, so the highlight is a band rather than a floating pill. */
    height: 100%;
  }
  /* The whole row is the target; a button laid over it keeps that keyboard-reachable without
     nesting interactive elements inside one another. */
  .hit {
    position: absolute; inset: 0; width: 100%; height: 100%;
    background: none; border: 0; padding: 0; margin: 0; cursor: pointer;
  }
  .cell { min-width: 0; display: flex; align-items: center; gap: var(--space-2); }
  /*
   * Pills sit against the graph, which is the thing they label, and are clipped to their own
   * column rather than spilling over the lanes. Right-aligning them looked tidier and read
   * worse: the ends of the names lined up, so what varied down the list was the left edge,
   * which is where the eye starts.
   */
  .cell.refs {
    flex-direction: column; align-items: flex-start; justify-content: center; gap: 1px;
    padding-left: var(--space-1); padding-right: var(--space-2);
    overflow: hidden;
  }
  .line { display: flex; align-items: center; gap: var(--space-1); max-width: 100%; min-width: 0; }
  .cell.message { gap: var(--space-3); }

  /*
   * A pill per ref, coloured by what kind of ref it is. The dot carries the colour and the
   * text stays near-black, because a whole pill in colour at 11px is unreadable and there can
   * be three of them on one row.
   */
  /* Sized so two stack inside one row: 12px of text and a hairline either side is 12.5px, and
     the row is 28px. Any larger and the second pill is cut off by the row below. */
  .pill {
    display: inline-flex; align-items: center; gap: 4px;
    flex: 0 1 auto; min-width: 0; font-size: 10px; line-height: 12px; padding: 0 6px;
    border-radius: 7px; border: 1px solid var(--border);
    background: var(--bg-1); color: var(--fg-1);
    max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .pip { width: 5px; height: 5px; }
  .pip {
    flex: 0 0 auto; width: 6px; height: 6px; border-radius: 50%;
    background: var(--fg-2);
  }
  .pill.local_branch .pip { background: var(--lane-1); }
  .pill.remote_branch { color: var(--fg-2); }
  .pill.remote_branch .pip { background: var(--fg-2); }
  .pill.tag .pip { background: var(--lane-3); }
  .pill.stash .pip { background: var(--lane-5); }
  /* The branch you are on: filled, since it is the one fact the row column exists to show. */
  .pill.head {
    background: var(--accent); border-color: var(--accent);
    color: var(--accent-fg); font-weight: 600;
  }
  .pill.head .pip { background: var(--accent-fg); }
  /* A control, not a label: the count opens the refs it stands for. */
  .pill.more {
    color: var(--fg-2); background: none; border-style: dashed; padding: 0 4px;
    flex: 0 0 auto; font: inherit; font-size: 10px; line-height: 12px; cursor: pointer;
    position: relative; z-index: 1;
  }
  .pill.more:hover { color: var(--fg-0); border-color: var(--border-strong); }

  /* The summary takes its natural width and the dimmed body absorbs what is left. Letting
     both shrink equally gave the body most of the row, so summaries were cut to a few
     characters while their continuation ran on — the wrong half was being kept. */
  /*
   * The summary takes its natural width and the body absorbs what is left. No percentage cap:
   * a percentage resolves against a containing block whose width is not obvious inside a grid
   * cell inside a button, and it cost the WIP row most of its text.
   *
   * The body has a zero basis, so it can never be the reason the summary has to shrink; it
   * only ever grows into space nothing else wanted. That is what stops the continuation
   * winning the row from the summary it continues.
   */
  .summary {
    flex: 0 1 auto; min-width: 0; color: var(--fg-0);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .detail {
    flex: 1 1 0; min-width: 0; color: var(--fg-2);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  /* Pushed to the trailing edge, so the two columns line up down the list whatever the
     summary before them happens to be. */
  .age {
    flex: 0 0 auto; margin-left: auto; color: var(--fg-2); text-align: right; font-size: 11px;
  }
  /*
   * The object id, set apart rather than just dimmed: it is the one field on the row nobody
   * reads as prose, and a tinted plate says so faster than a lighter grey does.
   */
  .sha {
    flex: 0 0 auto; color: var(--fg-2); font-size: 11px; letter-spacing: -0.01em;
    background: var(--bg-2); border-radius: var(--radius-1); padding: 0 5px; line-height: 16px;
  }

  /* The counts on the WIP row, in the same two colours the staging panel uses for them. */
  .tally { flex: 0 0 auto; font-size: 11px; font-variant-numeric: tabular-nums; }
  .tally.edit { color: var(--lane-1); }
  .tally.add { color: var(--ok); }

  .wip-node {
    width: 10px; height: 10px; border-radius: 50%;
    border: 2px dashed var(--fg-2); margin-left: var(--space-1);
  }

  .wip-panel {
    width: var(--details-w, 340px); flex: 0 0 auto; overflow-y: auto;
    border-left: 1px solid var(--border); background: var(--bg-1);
    padding: var(--space-3);
    /* The compose box is sticky against this, so the panel is what scrolls. */
    display: flex; flex-direction: column;
  }
</style>
