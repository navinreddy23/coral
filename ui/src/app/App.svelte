<script lang="ts">
  import { untrack } from 'svelte';
  import { messageOf } from '../ipc/error';
  import {
    covers,
    hasFlag,
    localRow,
    oidOf,
    RowFlag,
    widestLane,
    type Frame,
  } from '../graph/frame';
  import GraphCanvas from '../graph/GraphCanvas.svelte';
  import Splitter from './Splitter.svelte';
  import { PANE_LIMITS, PanesState } from '../state/panes.svelte';
  import DiffView from './DiffView.svelte';
  import { DiffState } from '../state/diff.svelte';
  import { HostingState } from '../state/hosting.svelte';
  import {
    commitDetail,
    openInBrowser,
    patchRangeSize,
    pickPatchFiles,
    type PullRequest,
  } from '../ipc/commands';
  import { ActionsState } from '../state/actions.svelte';
  import { CommitState } from '../state/commit.svelte';
  import MergeTool from './MergeTool.svelte';
  import NewRequest from './NewRequest.svelte';
  import { MergeState } from '../state/merge.svelte';
  import RebasePicker from './RebasePicker.svelte';
  import { RebaseState } from '../state/rebase.svelte';
  import Palette, { type Command } from './Palette.svelte';
  import StatusBar from './StatusBar.svelte';
  import Ask, { type Choice } from './Ask.svelte';
  import Preferences from './Preferences.svelte';
  import Menu, { type MenuItem } from './Menu.svelte';
  import HostMark, { hostOf } from './HostMark.svelte';
  import RefMark from './RefMark.svelte';
  import Toasts from './Toasts.svelte';
  import Splash from './Splash.svelte';
  import Start from './Start.svelte';
  import Remotes from './Remotes.svelte';
  import Activity from './Activity.svelte';
  import { ActivityState } from '../state/activity.svelte';
  import { StashesState } from '../state/stashes.svelte';
  import { StartState } from '../state/start.svelte';
  import { FindState } from '../state/find.svelte';
  import type { PlacedStash } from '../ipc/stash';
  import { ExperimentalState } from '../state/experimental.svelte';
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
  import { shortAge } from './age';
  import { initialsOf } from '../graph/initials';
  import type { Action } from '../ipc/commands';
  import {
    DEFAULT_METRICS,
    firstRowFor,
    fittedMetrics,
    GRAPH_COLUMN_PX,
    graphWidthFor,
    laneX,
    listTop,
    REFS_COLUMN_PX,
    spacerHeight,
  } from '../graph/layout';
  import {
    commitUrl,
    graphRowOf,
    initialRepo,
    open,
    pickDirectory,
    pickGitProgram,
    pickRepository,
  } from '../ipc/commands';
  import { GraphState } from '../state/graph.svelte';
  import { RefsState } from '../state/refs.svelte';
  import { ScopeState } from '../state/scope.svelte';
  import ChromeMark from './ChromeMark.svelte';
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
  import type {
    CommitMeta,
    RepoInfo,
    StatusEntry,
    Submodule,
    SubmoduleRevision,
  } from '../ipc/types';
  import { submoduleRevision } from '../ipc/commands';
  import type { PlacedRef } from '../state/refs.svelte';

  // Every remembered view choice lives here, so a toggle is a preference rather than a mode
  // the window forgets on the next launch.
  const views = new ViewsState();
  const graph = new GraphState();
  const theme = new ThemeState();
  const refs = new RefsState();
  const scope = new ScopeState();
  const selection = new SelectionState();
  const worktree = new WorktreeState();
  let showWip = $state(false);
  const tabs = new TabsState();
  let showHelp = $state(false);

  /** Which shortcuts actually do something today; the help overlay dims the rest. */
  const LIVE = new Set([
    'select.next', 'select.previous', 'select.first', 'select.last',
    'stage.all', 'unstage.all', 'tab.new', 'tab.close', 'tab.next', 'tab.previous',
    'palette', 'repo.open', 'terminal', 'search.commits',
    'panel.left', 'panel.detail', 'help', 'undo', 'redo',
    'commit', 'commit.stageAll', 'commit.focus', 'stage.file', 'unstage.file',
    'branch.create', 'fetch.all', 'toolbar', 'filter.focus',
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

    // Escape closes what is over the graph, wherever the focus went after it opened. The
    // search bar had this on its own field alone, so clicking a result — which is what the
    // bar is for — left no way to dismiss it but the mouse.
    if (event.key === 'Escape' && find.open) {
      find.close();
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
      case 'search.commits': openFind(); break;
      // The same two the toolbar runs. They were listed and rebindable but did nothing, which
      // for Ctrl+Z of all keys is worse than not offering it: the journal is what makes an
      // action on somebody's history safe to try, and the key everyone reaches for to undo one
      // was inert while the button beside it worked.
      case 'undo': toolbarAction('undo'); break;
      case 'redo': toolbarAction('redo'); break;
      case 'branch.create': toolbarAction('branch'); break;
      case 'fetch.all': toolbarAction('fetch'); break;
      case 'toolbar': views.set('toolbar', !views.current.toolbar); break;
      case 'filter.focus': filterTick += 1; break;
      case 'commit': void recordCommit(false); break;
      case 'commit.stageAll': void recordCommit(true); break;
      case 'commit.focus': focusCommitMessage(); break;
      case 'stage.file': void stageOpenFile(true); break;
      case 'unstage.file': void stageOpenFile(false); break;
      case 'terminal': terminal.toggle(); break;
      case 'repo.open': void openAnother(); break;
      default: break;
    }
  }

  /**
   * Opens the find bar and puts the caret in it.
   *
   * The field is created by this same change, so focusing it has to wait for the DOM.
   */
  function openFind() {
    find.show();
    queueMicrotask(() => findField?.focus());
    findField?.select();
  }

  let findField = $state<HTMLInputElement | null>(null);

  /** Steps to the next match, or the previous one, and scrolls it into view. */
  async function stepFind(direction: 1 | -1) {
    const row = find.step(direction);
    if (row !== null) await reveal(row);
  }

  function findKey(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      find.close();
      return;
    }
    if (event.key === 'Enter') {
      event.preventDefault();
      void stepFind(event.shiftKey ? -1 : 1);
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
  const commitDraft = new CommitState();
  /**
   * Ticking amend fills the message in from the commit being replaced.
   *
   * Amending is nearly always a fix to the change rather than to what it says, so an empty box
   * asks the user to retype a message they were not trying to alter — and one they can no
   * longer read, since the panel showing it is the one they are typing into.
   */
  let amending = false;
  $effect(() => {
    if (commitDraft.amend === amending) return;
    amending = commitDraft.amend;
    if (!amending) {
      commitDraft.unseed();
      return;
    }
    const path = info?.path;
    if (path === undefined || commitDraft.message !== '') return;
    void commitDetail(path, 'HEAD')
      .then((detail) => {
        // Still wanted: the tick can be undone, or the panel closed, while this is in flight.
        if (commitDraft.amend && commitDraft.message === '') {
          commitDraft.seed(detail.commit.summary, detail.commit.body);
        }
      })
      .catch(() => undefined);
  });
  /** Bumped to ask the branch panel for the caret; see the prop's own note. */
  let filterTick = $state(0);
  const merge = new MergeState();
  const hosting = new HostingState();

  /** The branch a pull or merge request is being opened for, or null. */
  let proposing = $state<string | null>(null);

  /** What the host calls one. GitHub says pull, GitLab says merge. */
  const requestWord = $derived(
    hosting.view?.host?.kind === 'gitlab' ? 'Merge request' : 'Pull request',
  );

  /**
   * Branches the host could merge into, by their bare names.
   *
   * From the remote rather than from the local list: a branch that exists only on this machine
   * is not something the host can be asked to merge into, and offering it would produce a
   * refusal from the API rather than an answer.
   */
  const remoteBranchNames = $derived([
    ...new Set(
      refs.groups.remote
        .map((r) => r.short.slice(r.short.indexOf('/') + 1))
        .filter((name) => name !== '' && name !== 'HEAD'),
    ),
  ].sort());
  const rebase = new RebaseState();
  const signing = new SigningState();
  const ssh = new SshState();
  const toasts = new ToastsState();
  const remotes = new RemotesState();
  const stashes = new StashesState();
  const startPage = new StartState();
  const find = new FindState();
  let showStart = $state(false);
  let showRemotes = $state<{ focus: string | null } | null>(null);
  const activity = new ActivityState();
  const experimental = new ExperimentalState();
  let showActivity = $state(false);
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
    /** Whether a line of text is wanted as well as a choice. Stated, never inferred. */
    asksText: boolean;
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
      asksText: true,
      placeholder: '',
      initial,
      choices: [{ id: 'ok', label: 'OK', primary: true }],
    });
    return choice === null ? null : text;
  }

  /** Asks before something that cannot be undone. */
  async function confirmThat(title: string, detail: string): Promise<boolean> {
    const { choice } = await ask({
      title,
      detail,
      asksText: false,
      placeholder: '',
      initial: '',
      choices: [{ id: 'yes', label: 'Yes', primary: true }],
    });
    return choice !== null;
  }

  let scroller = $state<HTMLDivElement | null>(null);

  const headName = $derived(
    info && info.head.kind !== 'detached' ? info.head.name : null,
  );

  /**
   * What HEAD is on, as one value that changes whenever the checkout does.
   *
   * The branch name alone cannot see a move between two commits with no branch on either:
   * checking out a commit while already detached left this unchanged, so the view stayed where
   * it was and the checkout looked as though it had not happened.
   */
  const headMark = $derived(
    info === null ? null : info.head.kind === 'detached' ? info.head.oid : info.head.name,
  );

  /**
   * The row HEAD sits on while it is detached, or null when it is on a branch.
   *
   * A detached HEAD has no ref to label its commit, so without this the row that is actually
   * checked out is drawn like every other row.
   */
  let detachedRow = $state<number | null>(null);

  $effect(() => {
    const head = info?.head;
    const path = info?.path;
    if (path === undefined || head === undefined || head.kind !== 'detached') {
      detachedRow = null;
      return;
    }
    const oid = head.oid;
    void graphRowOf(path, oid).then(
      (row) => {
        // The answer can arrive after another checkout has already moved HEAD on.
        if (info?.head.kind === 'detached' && info.head.oid === oid) detachedRow = row;
      },
      () => {
        detachedRow = null;
      },
    );
  });

  /**
   * The repositories that have a terminal pane, oldest first.
   *
   * A pane is built the first time the terminal is shown for a repository and then stays,
   * hidden while another tab is active: xterm holds the scrollback and the subscription to the
   * shell's output, and neither survives being unmounted.
   */
  let terminalsFor = $state<string[]>([]);

  $effect(() => {
    const path = info?.path;
    if (!terminal.open || path === undefined) return;
    if (!untrack(() => terminalsFor).includes(path)) terminalsFor = [...terminalsFor, path];
  });

  // A shell belongs to a repository, and a repository with no tab left has nowhere to show
  // one. Watching the session catches every way a tab goes — the close button, the group
  // close, the command — rather than each of them having to remember.
  $effect(() => {
    const open = new Set(tabs.session.tabs.map((t) => TabsState.workingPath(t)));
    void terminal.keepOnly(open);
    const kept = untrack(() => terminalsFor).filter((p) => open.has(p));
    if (kept.length !== untrack(() => terminalsFor).length) terminalsFor = kept;
  });

  /**
   * Selects a commit, or compares it with the one already selected.
   *
   * Ctrl or Shift, as the reference takes either: what the modifier means here is "and this
   * one too", not a range, so both do the same thing.
   */
  function pick(row: number, event?: MouseEvent) {
    const local = localRow(graph.frame, row);
    if (local === null || !graph.frame || !info) return;
    showWip = false;
    const oid = oidOf(graph.frame, local);
    const second = event !== undefined && (event.ctrlKey || event.metaKey || event.shiftKey);
    void (second ? selection.compare(info.path, row, oid) : selection.select(info.path, row, oid));
  }

  /*
   * A working-tree action supersedes whatever the line along the bottom last said.
   *
   * Staging, discarding and committing do not go through `actions`, so a failed checkout sat
   * along the bottom while a commit succeeded above it — the line describing a state the
   * repository was no longer in. Watched as one boolean rather than wired through the panel:
   * every one of those calls raises it.
   */
  let wasBusy = false;
  $effect(() => {
    const busy = worktree.busy;
    if (busy && !wasBusy) actions.clear();
    wasBusy = busy;
  });

  /** Reloads everything after an operation finished, since it may have moved any of it. */
  async function reloadAll() {
    if (!info) return;
    // The line along the bottom describes the action that stopped. Continuing or aborting ends
    // it, and leaving "rebase onto main stopped on conflicts" up after the rebase was aborted
    // says the repository is in a state it is no longer in.
    actions.clear();
    const path = info.path;
    await Promise.all([refs.load(path), worktree.load(path), merge.load(path)]);
    await graph.open(path);
  }

  /**
   * Records the commit the staging panel is holding, from the keyboard.
   *
   * The panel's own button does the same thing; this is the path Ctrl+Enter takes, and it has
   * to work whether or not the panel is on screen — the message survives the panel being
   * unmounted, so the shortcut should not be the one thing that needs it visible.
   */
  async function recordCommit(stageEverything: boolean) {
    if (!info || worktree.busy) return;
    if (stageEverything && worktree.unstaged.length > 0) {
      await worktree.stage(worktree.unstaged.map((f) => f.path), true);
    }
    if (!commitDraft.ready(worktree)) return;
    await worktree.commit(commitDraft.message, commitDraft.amend);
    if (!worktree.error) {
      commitDraft.clear();
      await reloadAll();
    }
  }

  /** Shows the working copy and puts the caret in the summary field. */
  function focusCommitMessage() {
    if (!info) return;
    pickWip();
    commitDraft.focus();
  }

  /**
   * Stages or unstages the file whose diff is open.
   *
   * "Current file" is the one being read, which is the only file the window has a notion of.
   * Without a diff open there is nothing to act on, and doing something to a file the user
   * cannot see would be worse than doing nothing.
   */
  async function stageOpenFile(stage: boolean) {
    const path = diff.path;
    if (!info || path === null || diff.source === 'commit' || diff.source === 'compare') return;
    await worktree.stage([path], stage);
    await diff.reload(info.path);
  }

  /**
   * The webview's own context menu, which Coral does not want and mostly did not ask about.
   *
   * Right-clicking anywhere this window has no menu of its own — the graph's empty space, a
   * panel's background, the dimmed backdrop of a dialog — brought up Back, Forward, Reload and
   * Inspect Element. Reload restarts the interface and takes a half-written commit message
   * with it, and Back navigates a page nobody using this knows is a page.
   *
   * Left alone in the two places the platform menu is the only way to reach the clipboard:
   * inside a field, and over selected text.
   */
  function platformMenu(event: MouseEvent) {
    const target = event.target as HTMLElement | null;
    if (target?.closest('input, textarea')) return;
    if ((window.getSelection()?.toString() ?? '') !== '') return;
    event.preventDefault();
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
  async function act(action: Action): Promise<boolean> {
    if (!info) return false;
    const path = info.path;
    const before = refSignature();
    const outcome = await actions.run(path, action);
    if (!outcome) {
      // `actions` keeps the message for the status line; the toast is what carries it to
      // someone who is not looking at the bottom of the window.
      if (actions.report) toasts.push('error', 'Something went wrong', actions.report.text);
      return false;
    }

    const said = describe(outcome.what, outcome.message, outcome.conflicted);
    toasts.push(said.kind, said.title, said.detail);

    // Not for a deletion. A refused delete is refused because the remote will not part with
    // that branch — it is protected, or it is the default one — and neither pulling it nor
    // forcing it changes that. Offering both would be offering to force a deletion, which is
    // not a thing anybody should be one click from.
    if (
      action.kind === 'push' &&
      !action.forceWithLease &&
      !action.delete &&
      wasRejected(outcome.message)
    ) {
      await offerToForce(action, outcome.message);
    }

    await Promise.all([refs.load(path), worktree.load(path), merge.load(path)]);
    const after = refSignature();
    if (before !== after) {
      await graph.open(path);
      // Again, after the walk. The row a ref carries belongs to the walk it was read from,
      // and an action that moves a ref usually changes the shape of the walk as well — a
      // fetch that brings one commit in shifts every row below it by one. Read before the
      // walk, as they have to be to tell whether anything moved at all, every pill then sat
      // one commit out. The watcher path has always done it in this order and says why.
      await Promise.all([refs.load(path), stashes.load(path)]);
    }

    // A checkout moves HEAD, and leaving the view where it was is the commonest way to end up
    // reading the branch that was just left.
    if (action.kind === 'checkout') {
      info = await open(path).catch(() => info);
      await focusHead();
    }
    return true;
  }

  /**
   * Whether git refused a ref rather than failing to reach the remote.
   *
   * Read from the porcelain summary rather than from the exit code, which is the same for a
   * rejection and for a server that would not answer. Only a rejection has a next move.
   */
  function wasRejected(message: string): boolean {
    return /\[rejected\]|non-fast-forward|fetch first|stale info/iu.test(message);
  }

  /**
   * What to do about a push git would not take.
   *
   * Offered from here because a toast cannot be acted on, and until now the rejection was the
   * end of the road: the window had no force at all, so the only way past it was a terminal.
   */
  async function offerToForce(action: Extract<Action, { kind: 'push' }>, message: string) {
    const { choice } = await ask({
      title: 'The remote refused it',
      detail:
        `${message.trim()}\n\n` +
        'The remote has commits this branch does not. Pulling brings them in and keeps them. ' +
        'Forcing replaces them with what is here, and git allows it only while nobody else has ' +
        'moved the branch since Coral last fetched it.',
      asksText: false,
      placeholder: '',
      initial: '',
      choices: [
        { id: 'pull', label: 'Pull and rebase', primary: true },
        { id: 'force', label: 'Force push' },
      ],
    });
    if (choice === 'pull') {
      await act({ kind: 'pull', remote: action.remote, mode: 'rebase' });
      return;
    }
    if (choice === 'force') await act({ ...action, forceWithLease: true });
  }

  /** The row under the pointer, if the loaded frame reaches it. */
  function rightClickRow(event: MouseEvent, row: number) {
    const local = localRow(graph.frame, row);
    if (local === null || !graph.frame) return;
    commitMenu(event, row, oidOf(graph.frame, local));
  }

  /**
   * Checking out what is on a row, by name.
   *
   * A local branch is checked out as itself. A tracking branch is checked out under its own
   * name without the remote in front, which is git's own rule and makes a local branch that
   * follows it. A tag has no branch to be on, so git detaches, and the menu says so rather than
   * leaving the user to discover it.
   */
  function checkoutsFor(row: number): MenuItem[] {
    const here = refs.byRow.get(row) ?? [];
    const alongside = new Set(
      here.filter((r) => r.kind.kind === 'local_branch').map((r) => r.short),
    );
    const out: MenuItem[] = [];

    for (const ref of here) {
      if (ref.kind.kind === 'local_branch') {
        if (ref.short === headName) continue;
        out.push({
          kind: 'item',
          label: `Checkout ${ref.short}`,
          run: () => void goTo(ref),
        });
      } else if (ref.kind.kind === 'remote_branch') {
        const name = trackingName(ref);
        // Not when the local branch of that name is on this very row and already offered
        // above: two entries that read the same and do the same is a menu nobody can answer.
        // A local of that name sitting elsewhere is a different matter — that entry is how
        // the two get reconciled, and `goTo` asks which way.
        if (name === '' || name === headName || alongside.has(name)) continue;
        out.push({
          kind: 'item',
          label: `Checkout ${name}`,
          hint: `tracking ${ref.short}`,
          run: () => void goTo(ref),
        });
      } else if (ref.kind.kind === 'tag') {
        out.push({
          kind: 'item',
          label: `Checkout ${ref.short}`,
          hint: 'detaches HEAD',
          run: () => void act({ kind: 'checkout', rev: ref.short }),
        });
      }
    }
    return out.length === 0 ? out : [...out, { kind: 'separator' }];
  }

  /**
   * Bringing what is on a row into the current branch, or putting the branch on top of it.
   *
   * The same four the ref panel offers, because a commit in the graph is where people reach
   * for them: the row is right there and the branch panel is not. Named for a ref on the row
   * when there is one, since that is what the user is looking at; a row with no label is named
   * by its commit, which merges and rebases just as well.
   */
  function combineFor(row: number, oid: string): MenuItem[] {
    const here = refs.byRow.get(row) ?? [];
    // Nothing to do with the row HEAD is already on, and nothing sensible to say about a
    // detached HEAD, which has no branch to move.
    if (headName === null || here.some((r) => r.short === headName)) return [];

    const named =
      here.find((r) => r.kind.kind === 'local_branch') ??
      here.find((r) => r.kind.kind === 'remote_branch') ??
      here.find((r) => r.kind.kind === 'tag');
    const rev = named?.short ?? oid.slice(0, 8);
    const busy = actions.busy || worktree.busy;

    return [
      {
        kind: 'item',
        label: `Fast-forward ${headName} to ${rev}`,
        hint: 'refuses to merge',
        disabled: busy,
        run: () => void act({ kind: 'merge', rev, mode: 'ffOnly' }),
      },
      {
        kind: 'item',
        label: `Merge ${rev} into ${headName}`,
        disabled: busy,
        run: () => void act({ kind: 'merge', rev, mode: 'auto' }),
      },
      {
        kind: 'item',
        label: `Rebase ${headName} onto ${rev}`,
        disabled: busy,
        run: () => void act({ kind: 'rebase', onto: rev }),
      },
      {
        kind: 'item',
        label: `Rebase ${headName} onto ${rev}, interactively`,
        disabled: busy,
        run: () => info && void rebase.load(info.path, rev),
      },
      { kind: 'separator' },
    ];
  }

  /**
   * What can be done with one branch or tag, from the panel that lists them.
   *
   * The same three things people reach for — go to it, bring it in, put this work on top of it
   * — plus getting rid of it. A tag is checked out like anything else; git detaches for one,
   * and saying so is better than a menu that quietly does something else.
   */
  function refMenu(event: MouseEvent, ref: PlacedRef) {
    event.preventDefault();
    const head = headName ?? 'HEAD';
    const current = ref.short === headName;
    const busy = actions.busy || worktree.busy;
    const items: MenuItem[] = [];

    if (ref.row !== null) {
      items.push({
        kind: 'item',
        label: 'Show it in the graph',
        run: () => ref.row !== null && void reveal(ref.row),
      });
    }

    if (!current) {
      const [label, hint] = checkoutOf(ref);
      items.push({ kind: 'item', label, hint, disabled: busy, run: () => void goTo(ref) });
    }

    if (ref.kind.kind === 'local_branch' && hosting.available) {
      items.push({ kind: 'separator' });
      items.push({
        kind: 'item',
        label: `Open a ${requestWord.toLowerCase()} from ${ref.short}`,
        disabled: busy,
        run: () => (proposing = ref.short),
      });
    }

    if (!current && ref.kind.kind !== 'stash') {
      items.push({ kind: 'separator' });
      items.push({
        kind: 'item',
        label: `Fast-forward ${head} to ${ref.short}`,
        hint: 'refuses to merge',
        disabled: busy,
        run: () => void act({ kind: 'merge', rev: ref.short, mode: 'ffOnly' }),
      });
      items.push({
        kind: 'item',
        label: `Merge ${ref.short} into ${head}`,
        disabled: busy,
        run: () => void act({ kind: 'merge', rev: ref.short, mode: 'auto' }),
      });
      items.push({
        kind: 'item',
        label: `Rebase ${head} onto ${ref.short}`,
        disabled: busy,
        run: () => void act({ kind: 'rebase', onto: ref.short }),
      });
      items.push({
        kind: 'item',
        label: `Rebase ${head} onto ${ref.short}, interactively`,
        disabled: busy,
        run: () => info && void rebase.load(info.path, ref.short),
      });
    }

    // Everything that can be done with the commit the ref is on, as the reference offers it:
    // a branch label is a place in the history as much as it is a name.
    const oid = ref.peeled ?? ref.target;
    items.push({ kind: 'separator' });
    items.push({ kind: 'item', label: 'Create branch here', run: () => void branchAt(oid) });
    items.push({ kind: 'item', label: 'Cherry pick commit…', run: () => void cherryPick(oid) });
    items.push({
      kind: 'item',
      label: `Reset ${head} to this commit`,
      run: () => void confirmHardReset(oid, head),
      danger: true,
      disabled: busy,
    });
    items.push({
      kind: 'item',
      label: 'Revert commit',
      disabled: busy,
      run: () => void act({ kind: 'revert', revs: [oid] }),
    });

    // What the graph is drawn from, which is a different question from what can be done to the
    // branch, and so sits in its own group.
    items.push({ kind: 'separator' });
    if (scope.solo === ref.name) {
      items.push({
        kind: 'item',
        label: 'Leave solo',
        hint: 'show every branch again',
        run: () => void soloRef(null),
      });
    } else {
      items.push({
        kind: 'item',
        label: `Solo ${ref.short}`,
        hint: 'walk only this one',
        run: () => void soloRef(ref),
      });
    }
    items.push({
      kind: 'item',
      label: scope.hidden.includes(ref.name) ? `Show ${ref.short}` : `Hide ${ref.short}`,
      // No hint. What hiding actually does — leave the commits another branch still reaches —
      // does not fit on a menu row, and the truncated half of it says the opposite.
      run: () => void toggleHidden(ref),
    });

    items.push({ kind: 'separator' });
    items.push({
      kind: 'item',
      label: ref.kind.kind === 'tag' ? 'Copy the tag name' : 'Copy the branch name',
      hint: ref.short,
      run: () => void copyName(ref.short),
    });
    items.push({ kind: 'item', label: 'Copy commit sha', run: () => void copySha(oid) });

    if (ref.kind.kind === 'local_branch' && !current) {
      items.push({ kind: 'separator' });
      items.push({
        kind: 'item',
        label: `Delete ${ref.short}…`,
        danger: true,
        disabled: busy,
        run: () => void deleteBranch(ref.short),
      });
    } else if (ref.kind.kind === 'remote_branch') {
      items.push({ kind: 'separator' });
      items.push({
        kind: 'item',
        label: `Delete ${ref.short} from the remote…`,
        danger: true,
        disabled: busy,
        run: () => void deleteRemoteBranch(ref.short),
      });
    } else if (ref.kind.kind === 'tag') {
      items.push({ kind: 'separator' });
      items.push(...pushTagItems(ref.short));
      items.push({
        kind: 'item',
        label: `Delete the tag ${ref.short}…`,
        danger: true,
        disabled: busy,
        run: () => void deleteTag(ref.short),
      });
    }

    if (items.length === 0) return;
    menu = { x: event.clientX, y: event.clientY, items };
  }

  /**
   * Removes a branch from the remote it lives on.
   *
   * The remote and the branch are split out of the tracking name, since git wants them apart:
   * `git push github --delete probe/one`, not the name the panel shows. Only the leading
   * segment is the remote; the rest is the branch, however many slashes it has of its own.
   */
  async function deleteRemoteBranch(tracking: string) {
    const at = tracking.indexOf('/');
    if (at < 0) return;
    const remote = tracking.slice(0, at);
    const branch = tracking.slice(at + 1);
    const { choice } = await ask({
      title: `Delete ${branch} from ${remote}?`,
      detail:
        'It goes for everyone, not only here. Anyone who has already fetched it keeps their ' +
        'own copy, and any commit only this branch reached is left with no name on it. ' +
        'Nothing local is touched.',
      asksText: false,
      placeholder: '',
      initial: '',
      choices: [{ id: 'delete', label: `Delete ${branch}` }],
    });
    if (choice !== 'delete') return;
    await act({
      kind: 'push',
      remote,
      setUpstream: false,
      refspec: branch,
      tags: false,
      forceWithLease: false,
      delete: true,
    });
  }

  /**
   * Where a push should go when nobody has said: `origin` if it is there, else the only one.
   *
   * Null when there are several and none is called `origin`, which is the case that has to be
   * asked about rather than guessed at.
   */
  const defaultRemote = $derived(
    remotes.list.find((r) => r.name === 'origin')?.name ??
      (remotes.list.length === 1 ? remotes.list[0]?.name ?? null : null),
  );

  /**
   * Sends one tag to a remote.
   *
   * Tags do not travel with a push; git sends them only when they are named, which is why a
   * tag made in the window sat there looking published and was on nobody else's machine.
   */
  function pushTag(name: string, remote: string) {
    void act({
      kind: 'push',
      remote,
      setUpstream: false,
      refspec: `refs/tags/${name}`,
      tags: false,
      forceWithLease: false,
      delete: false,
    });
  }

  /** The menu items for pushing one tag: one remote, or a choice of them. */
  function pushTagItems(name: string): MenuItem[] {
    if (remotes.list.length === 0) return [];
    const busy = actions.busy || worktree.busy;
    // One remote is one item; more than one is a choice, whether or not one of them is called
    // origin. Falling back to the default when there were several made every other remote
    // unreachable for tags — a repository with a mirror could push a tag to origin and had no
    // way at all to send it anywhere else.
    if (remotes.list.length === 1) {
      const remote = remotes.list[0]?.name ?? 'origin';
      return [
        {
          kind: 'item',
          label: `Push ${name} to ${remote}`,
          disabled: busy,
          run: () => pushTag(name, remote),
        },
      ];
    }
    return [
      {
        kind: 'submenu',
        label: `Push ${name}`,
        items: [...remotes.list]
          .sort((a, b) => Number(b.name === defaultRemote) - Number(a.name === defaultRemote))
          .map((r) => ({
            kind: 'item' as const,
            label: r.name,
            disabled: busy,
            run: () => pushTag(name, r.name),
          })),
      },
    ];
  }

  /** The branch name a remote-tracking ref checks out as: `origin/topic` becomes `topic`. */
  function trackingName(ref: PlacedRef): string {
    return ref.short.slice(ref.short.indexOf('/') + 1);
  }

  /**
   * Goes to a ref, asking first when a remote branch already has a local branch of its name.
   *
   * `git checkout topic` for `origin/topic` lands on the existing local `topic`, wherever that
   * happens to be. When the two have diverged that is not what asking for the remote one looks
   * like it does, and the local branch quietly wins. So the choice is put to the user, as
   * GitKraken does: go to the local branch as it stands, or move it onto the remote first.
   */
  async function goTo(ref: PlacedRef) {
    const name = trackingName(ref);
    if (ref.kind.kind !== 'remote_branch' || name === '') {
      await act({ kind: 'checkout', rev: ref.short });
      return;
    }
    const local = refs.groups.local.find((r) => r.short === name);
    if (!local || local.target === ref.target) {
      await act({ kind: 'checkout', rev: name });
      return;
    }

    const { choice } = await ask({
      title: `${name} is already here`,
      detail: divergence(local, ref),
      asksText: false,
      placeholder: '',
      initial: '',
      choices: [
        { id: 'checkout', label: `Checkout ${name}`, primary: true },
        { id: 'reset', label: `Reset ${name} to ${ref.short}` },
      ],
    });
    if (choice === null) return;
    // The reset only ever runs on a branch the checkout confirmed we are on. Ordering it the
    // other way, or running it regardless, resets whatever HEAD was left on when the checkout
    // failed — which is the one outcome nobody asked for.
    if (!(await act({ kind: 'checkout', rev: name })) || choice !== 'reset') return;
    await act({ kind: 'reset', rev: ref.short, mode: 'hard' });
  }

  /** How the local branch and its remote differ, in the words the reset dialog needs. */
  function divergence(local: PlacedRef, ref: PlacedRef): string {
    if (local.upstream === ref.short && local.ahead === 0 && local.behind > 0) {
      const many = local.behind === 1 ? 'commit' : 'commits';
      return (
        `${local.short} is ${local.behind} ${many} behind ${ref.short} and has nothing of its ` +
        'own. Resetting brings it up to date; uncommitted changes in the working copy are ' +
        'discarded with it.'
      );
    }
    return (
      `The local ${local.short} and ${ref.short} are on different commits. Checking out goes to ` +
      'the local branch as it stands. Resetting moves it onto the remote, and any commit only ' +
      'the local branch reached is left with no name on it.'
    );
  }

  /** How one ref is checked out: what to call the menu item, and what it will do. */
  function checkoutOf(ref: PlacedRef): [string, string | undefined] {
    if (ref.kind.kind === 'remote_branch') {
      return [`Checkout ${trackingName(ref)}`, `tracking ${ref.short}`];
    }
    if (ref.kind.kind === 'tag') {
      return [`Checkout ${ref.short}`, 'detaches HEAD'];
    }
    return [`Checkout ${ref.short}`, undefined];
  }

  async function deleteBranch(name: string) {
    const { choice } = await ask({
      title: `Delete ${name}?`,
      detail:
        'The branch goes; the commits on it stay until git collects them, and are hard to find ' +
        'again without it. Anything only this branch reached is effectively gone.',
      asksText: false,
      placeholder: '',
      initial: '',
      choices: [{ id: 'delete', label: `Delete ${name}` }],
    });
    if (choice !== 'delete') return;
    await act({ kind: 'branchDelete', name, force: true });
  }

  async function deleteTag(name: string) {
    const { choice } = await ask({
      title: `Delete the tag ${name}?`,
      detail: 'The tag goes here. A copy on a remote stays until it is deleted there too.',
      asksText: false,
      placeholder: '',
      initial: '',
      choices: [{ id: 'delete', label: `Delete ${name}` }],
    });
    if (choice !== 'delete') return;
    await act({ kind: 'tagDelete', name });
  }

  /**
   * Cherry picking, and whether to record it straight away.
   *
   * Two different things, and the reference asks rather than choosing: committing lands it on
   * this branch now, while stopping short leaves it staged so it can be changed, split, or
   * folded into something else first.
   */
  async function cherryPick(oid: string) {
    const { choice } = await ask({
      title: 'Commit the cherry picked changes?',
      detail:
        'Yes records a commit on this branch straight away.\n' +
        'No leaves the changes staged, so they can be edited, split, or folded into ' +
        'something else before anything is recorded.',
      asksText: false,
      placeholder: '',
      initial: '',
      choices: [
        { id: 'yes', label: 'Yes', primary: true },
        { id: 'no', label: 'No' },
      ],
    });
    if (choice === null) return;
    await act({ kind: 'cherryPick', revs: [oid], commit: choice === 'yes' });
  }

  /**
   * What right-clicking a commit offers.
   *
   * Grouped as the reference groups them: where to go, what to bring in, what to make here,
   * how to rewrite the history, what to copy, and what to tag. The history edits are refused
   * by the engine for a commit that is not on this branch or for a range holding a merge, so
   * nothing here has to guess at whether they are safe.
   */
  function commitMenu(event: MouseEvent, row: number, oid: string) {
    event.preventDefault();
    pick(row);
    const short = oid.slice(0, 8);
    const branch = headName ?? 'HEAD';
    const summary = visibleMeta.get(row)?.summary ?? '';

    menu = {
      x: event.clientX,
      y: event.clientY,
      items: [
        // The refs on this row first, each checked out by its own name. Checking the commit out
        // is a different act with a different result — a detached HEAD — and offering only that
        // meant every checkout from the graph detached, whatever branch was sitting on the row.
        ...checkoutsFor(row),
        {
          kind: 'item',
          label: 'Checkout this commit',
          hint: `${short}, detached`,
          run: () => void act({ kind: 'checkout', rev: oid }),
        },
        { kind: 'item', label: 'Create worktree from this commit', run: () => void worktreeAt(oid) },
        { kind: 'separator' },
        ...combineFor(row, oid),
        { kind: 'item', label: 'Create branch here', run: () => void branchAt(oid) },
        {
          kind: 'item',
          label: 'Cherry pick commit…',
          run: () => void cherryPick(oid),
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
          label: 'Interactive rebase from this commit',
          hint: `${short} and newer`,
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
        {
          kind: 'item',
          label: 'Apply a patch file…',
          hint: 'onto this branch',
          disabled: actions.busy,
          run: () => void applyPatch(),
        },
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

  /**
   * Applies patch files somebody sent.
   *
   * The choice is the same one cherry picking asks, and for the same reason: a patch is either
   * a commit you want on this branch now, or a change you want to look at before anything is
   * recorded. The file dialog takes several, because a series is several files.
   */
  async function applyPatch() {
    if (!info) return;
    const files = await pickPatchFiles('Which patch files should be applied?');
    if (files.length === 0) return;
    const { choice } = await ask({
      title: files.length === 1 ? 'Commit the patch?' : `Commit the ${files.length} patches?`,
      detail:
        'Yes records a commit for each patch, keeping the author and message it carries.\n' +
        'No leaves the changes in the working copy, so they can be read and edited before ' +
        'anything is recorded.',
      asksText: false,
      placeholder: '',
      initial: '',
      choices: [
        { id: 'yes', label: 'Yes', primary: true },
        { id: 'no', label: 'No' },
      ],
    });
    if (choice === null) return;
    await act({ kind: 'applyPatch', files, commit: choice === 'yes' });
  }

  /**
   * What the P button does, which depends on what is selected.
   *
   * Two commits picked is a request to export what lies between them; anything else is a
   * request to take a patch in. One button because they are the two halves of the same
   * exchange, and the selection says plainly which half is meant.
   */
  /** Past this many, exporting a range is worth confirming rather than just doing. */
  const MANY_PATCHES = 50;

  async function patchButton() {
    if (!info) return;
    const pair = selection.pair;
    if (pair === null) {
      await applyPatch();
      return;
    }
    // How many, before a directory is chosen. Two commits picked far apart is a file per
    // commit between them, and on a repository the size of the kernel that is very nearly a
    // million and a half files written into whichever folder was clicked.
    const count = await patchRangeSize(info.path, pair.from.oid, pair.to.oid).catch(() => -1);
    if (count === 0) {
      toasts.push('warn', 'Nothing to write', 'There are no commits between those two.');
      return;
    }
    if (count > MANY_PATCHES) {
      const { choice } = await ask({
        title: `Write about ${count.toLocaleString()} patch files?`,
        detail:
          'About, because a merge has no single patch and is skipped: that is the number of ' +
          'commits between the two you picked, and the number of files will be that or fewer. ' +
          'Picking two commits far apart on a large repository can be a great many of them.',
        asksText: false,
        placeholder: '',
        initial: '',
        // One choice: the dialog carries its own Cancel, and offering a second one beside it
        // put two buttons saying Cancel next to each other.
        choices: [{ id: 'yes', label: 'Write them' }],
      });
      if (choice !== 'yes') return;
    }

    const where = await pickDirectory('Where should the patches be written?');
    if (where === null) return;
    await act({ kind: 'patch', rev: pair.to.oid, from: pair.from.oid, directory: where });
  }

  async function patchOf(oid: string) {
    const where = await pickDirectory('Where should the patch be written?');
    if (where === null) return;
    await act({ kind: 'patch', rev: oid, from: null, directory: where });
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
      asksText: false,
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
      asksText: false,
      placeholder: '',
      initial: '',
      choices: [{ id: 'reset', label: 'Discard and reset', primary: true }],
    });
    if (choice === null) return;
    await act({ kind: 'reset', rev: oid, mode: 'hard' });
  }

  async function copyName(name: string) {
    if (await copyText(name)) toasts.push('ok', 'Copied', name);
    else toasts.push('error', 'Could not reach the clipboard');
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
            label: checkoutOf(r)[0],
            run: () => void goTo(r),
          },
        ],
      })),
    };
  }

  /**
   * Throws away working-tree changes, once the user has agreed to exactly what goes.
   *
   * The dialog splits the files rather than the engine, because the two halves are not the
   * same promise. A tracked file goes back to what HEAD holds and its content is still in the
   * object database; a file git has never seen is deleted from disk and exists nowhere else.
   * So the count of each is named, deleting the new ones is a separate button, and neither is
   * what the Enter key does.
   */
  async function discardChanges(entries: StatusEntry[]) {
    if (!info || entries.length === 0) return;
    const untracked = entries.filter((e) => e.worktree === 'untracked').map((e) => e.path);
    const tracked = entries.filter((e) => e.worktree !== 'untracked').map((e) => e.path);

    // Nothing here is primary, so Enter dismisses rather than discarding.
    const choices: Choice[] = [];
    if (tracked.length > 0) {
      choices.push({
        id: 'tracked',
        label: untracked.length > 0
          ? `Discard ${count(tracked.length, 'change')}, keep the new files`
          : `Discard ${count(tracked.length, 'change')}`,
      });
    }
    if (untracked.length > 0) {
      choices.push({
        id: 'all',
        label: tracked.length > 0
          ? `Discard everything, deleting ${count(untracked.length, 'new file')}`
          : `Delete ${count(untracked.length, 'new file')}`,
      });
    }

    const { choice } = await ask({
      title: 'Discard changes?',
      detail: discardDetail(tracked.length, untracked.length, headName),
      asksText: false,
      placeholder: '',
      initial: '',
      choices,
    });
    if (choice === null) return;

    await worktree.discard(tracked, choice === 'all' ? untracked : []);
    if (!worktree.error) toasts.push('ok', 'Changes discarded.');
  }

  /** "1 change", "4 changes" — the plural of a count without a helper library. */
  function count(n: number, noun: string): string {
    return `${n} ${noun}${n === 1 ? '' : 's'}`;
  }

  function discardDetail(tracked: number, untracked: number, branch: string | null): string {
    const where = branch === null ? 'the commit that is checked out' : branch;
    const parts: string[] = [];
    if (tracked > 0) {
      const goes = tracked === 1 ? 'goes' : 'go';
      parts.push(`${count(tracked, 'file')} ${goes} back to what ${where} last committed.`);
    }
    if (untracked > 0) {
      parts.push(
        `${count(untracked, 'file')} ${untracked === 1 ? 'is' : 'are'} not tracked by git,` +
          ' so deleting them removes the only copy there is.',
      );
    }
    parts.push('This cannot be undone.');
    return parts.join(' ');
  }

  /**
   * What can be done with one stash.
   *
   * Three separate things, and only applying is reversible: popping removes the entry once it
   * has landed, and dropping removes it without landing it at all. So dropping asks first and
   * says what it is about to lose, and neither destructive one is the primary.
   */
  function stashMenu(event: MouseEvent, stash: PlacedStash) {
    event.preventDefault();
    menu = {
      x: event.clientX,
      y: event.clientY,
      items: [
        {
          kind: 'item',
          label: 'Apply it, and keep it',
          hint: 'the stash stays on the stack',
          disabled: worktree.busy || actions.busy,
          run: () => void act({ kind: 'stashApply', index: stash.index, pop: false }),
        },
        {
          kind: 'item',
          label: 'Pop it',
          hint: 'apply it and take it off the stack',
          disabled: worktree.busy || actions.busy,
          run: () => void act({ kind: 'stashApply', index: stash.index, pop: true }),
        },
        { kind: 'separator' },
        {
          kind: 'item',
          label: 'Drop it…',
          hint: 'throw it away without applying it',
          disabled: worktree.busy || actions.busy,
          run: () => void dropStash(stash),
        },
      ],
    };
  }

  async function dropStash(stash: PlacedStash) {
    const { choice } = await ask({
      title: `Drop ${stash.name}?`,
      detail:
        `${stash.message}\n\nThe changes in this stash are not in any commit and not in the ` +
        'working copy. Dropping it is the only copy gone. This cannot be undone.',
      asksText: false,
      placeholder: '',
      initial: '',
      // Nothing is primary, so Enter does not drop it.
      choices: [{ id: 'drop', label: `Drop ${stash.name}` }],
    });
    if (choice !== 'drop') return;
    await act({ kind: 'stashDrop', index: stash.index });
  }

  function openActivity() {
    if (info) activity.repo(info.path);
    showActivity = true;
    void activity.load();
  }

  function openPreferences() {
    showPrefs = true;
    void experimental.load();
    if (!info) return;
    void signing.load(info.path);
    void ssh.load(info.path);
  }

  /** Points Coral at a git of the user's choosing, from the Experimental page. */
  async function chooseGitProgram() {
    const path = await pickGitProgram();
    if (path === null) return;
    await experimental.chooseGit({ kind: 'custom', path });
  }

  /** How a pull should integrate, offered at the caret beside the Pull button. */
  /** What a push can send beyond the current branch. Tags are the whole of it. */
  function pushMenu(event: MouseEvent) {
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const busy = actions.busy || worktree.busy;
    const tags = refs.groups.tags;
    menu = {
      x: box.left,
      y: box.bottom + 2,
      items: [
        {
          kind: 'item',
          label: 'Push this branch',
          hint: 'sets the upstream',
          disabled: busy,
          run: () =>
            void act({
              kind: 'push',
              remote: null,
              setUpstream: true,
              refspec: null,
              tags: false,
              forceWithLease: false,
              delete: false,
            }),
        },
        {
          kind: 'item',
          label: 'Force push this branch',
          hint: 'with lease',
          danger: true,
          disabled: busy,
          run: () => void confirmForcePush(),
        },
        {
          kind: 'item',
          label: 'Push this branch and every tag',
          hint: tags.length === 0 ? 'no tags here' : `${tags.length} tag${tags.length === 1 ? '' : 's'}`,
          disabled: busy || tags.length === 0,
          run: () =>
            void act({
              kind: 'push',
              remote: null,
              setUpstream: true,
              refspec: null,
              tags: true,
              forceWithLease: false,
              delete: false,
            }),
        },
      ],
    };
  }

  /**
   * Forcing on purpose, rather than after git has already said no.
   *
   * Asked for every time. `--force-with-lease` refuses when the remote has moved since Coral
   * last fetched, which is the case that would destroy somebody else's work — but it does not
   * refuse when the commits being replaced are only the user's own, and that is worth a
   * sentence before it happens rather than a toast afterwards.
   */
  async function confirmForcePush() {
    const { choice } = await ask({
      title: `Force push ${headName ?? 'this branch'}?`,
      detail:
        'The remote branch is replaced by this one. Commits on it that are not here are lost ' +
        'to anyone who has not already fetched them. git refuses if the remote has moved since ' +
        'Coral last fetched it, so this cannot overwrite a change it has not seen.',
      asksText: false,
      placeholder: '',
      initial: '',
      choices: [{ id: 'force', label: 'Force push' }],
    });
    if (choice !== 'force') return;
    await act({
      kind: 'push',
      remote: null,
      setUpstream: true,
      refspec: null,
      tags: false,
      forceWithLease: true,
      delete: false,
    });
  }

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
          label: `Push ${headName ?? 'this branch'} to ${remote}`,
          // Named rather than left to git. `git push <remote>` with no refspec asks git to
          // work out which branch, and with no upstream on that remote it answers "the
          // current branch has no upstream branch" instead of pushing — which is every push
          // to a second remote until one exists.
          hint: 'the upstream stays where it is',
          disabled: headName === null || actions.busy || worktree.busy,
          run: () =>
            void act({
              kind: 'push',
              remote,
              setUpstream: false,
              refspec: headName,
              tags: false,
              forceWithLease: false,
              delete: false,
            }),
        },
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
      asksText: false,
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

  /**
   * What can be done with one file in the working tree.
   *
   * Deleting is the only thing here that leaves nothing behind, so it is set apart and asked
   * about first. Everything else is reversible with the button beside it.
   */
  function fileMenu(event: MouseEvent, entry: StatusEntry, staged: boolean) {
    event.preventDefault();
    const busy = worktree.busy;
    const items: MenuItem[] = [
      {
        kind: 'item',
        label: 'Open the diff',
        run: () => openWorkingFile(entry.path, staged),
      },
      { kind: 'separator' },
      staged
        ? {
            kind: 'item',
            label: 'Unstage it',
            disabled: busy,
            run: () => void worktree.stage([entry.path], false),
          }
        : {
            kind: 'item',
            label: 'Stage it',
            disabled: busy,
            run: () => void worktree.stage([entry.path], true),
          },
      {
        kind: 'item',
        label: 'Discard its changes',
        hint: 'back to the last commit',
        disabled: busy,
        danger: true,
        run: () => void discardChanges([entry]),
      },
      {
        kind: 'item',
        label: 'Delete the file',
        disabled: busy,
        danger: true,
        run: () => void confirmDelete(entry),
      },
    ];
    menu = { x: event.clientX, y: event.clientY, items };
  }

  /**
   * Stages, unstages or discards part of the file on screen.
   *
   * Discarding asks first: it is the only one of the three that throws work away, and a hunk
   * is small enough to click by accident.
   */
  async function applyPart(
    part: 'stage' | 'unstage' | 'discard',
    hunk: number,
    lines: number[],
  ) {
    const file = diff.path;
    if (!info || file === null) return;

    if (part === 'discard') {
      const what = lines.length === 0 ? 'this hunk' : count(lines.length, 'line');
      const { choice } = await ask({
        title: `Discard ${what}?`,
        detail: `${file}\n\nThe change goes back to what is committed. It is not in any commit, ` +
          'so there is nothing to bring it back from.',
        asksText: false,
        placeholder: '',
        initial: '',
        choices: [{ id: 'discard', label: `Discard ${what}` }],
      });
      if (choice !== 'discard') return;
    }

    await worktree.applyPart(file, part, hunk, lines);
    if (worktree.error) {
      toasts.push('error', 'Could not apply that', worktree.error);
      return;
    }
    // The file's diff on this side is a different diff now, and may be empty.
    await diff.reload(info.path);
  }

  /** Deleting a file cannot be undone, so it is asked about by name. */
  async function confirmDelete(entry: StatusEntry) {
    const untracked = entry.worktree === 'untracked';
    const { choice } = await ask({
      title: `Delete ${entry.path}?`,
      detail: untracked
        ? 'The file is removed from the working tree. It is in no commit, so there is nothing ' +
          'to bring it back from.'
        : 'The file is removed from the working tree and its deletion staged. Committing that ' +
          'makes it permanent; until then the last commit still has it.',
      asksText: false,
      placeholder: '',
      initial: '',
      choices: [{ id: 'delete', label: `Delete ${entry.path}` }],
    });
    if (choice !== 'delete') return;
    await worktree.delete(untracked ? [] : [entry.path], untracked ? [entry.path] : []);
  }

  /** Opens one of the selected commit's files in the diff viewer, or one of the pair's. */
  function openFile(file: string) {
    if (!info) return;
    const pair = selection.pair;
    if (pair !== null) {
      void diff.openCompare(info.path, pair.from.oid, pair.to.oid, file);
      return;
    }
    const rev = selection.detail?.commit.oid;
    if (rev === undefined) return;
    void diff.open(info.path, rev, file);
  }

  /** Drops the comparison and goes back to the newer of the two on its own. */
  function clearCompare() {
    const pair = selection.pair;
    if (!info || pair === null) return;
    diff.close();
    void selection.select(info.path, pair.to.row, pair.to.oid);
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
      { id: 'push', label: 'Push', group: 'Remote', run: () => void act({ kind: 'push', remote: null, setUpstream: true, refspec: null, tags: false, forceWithLease: false, delete: false }) },
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
      {
        id: 'activity',
        label: 'Activity logs',
        group: 'View',
        run: openActivity,
      },
    ];

    for (const r of refs.groups.local) {
      if (r.short === headName) continue;
      out.push({ id: `co:${r.name}`, label: `Checkout ${r.short}`, group: 'Branch', run: () => void act({ kind: 'checkout', rev: r.short }) });
      out.push({ id: `merge:${r.name}`, label: `Merge ${r.short} into ${headName ?? 'HEAD'}`, group: 'Branch', run: () => void act({ kind: 'merge', rev: r.short, mode: 'auto' }) });
      out.push({ id: `rebase:${r.name}`, label: `Rebase onto ${r.short}`, group: 'Branch', run: () => void act({ kind: 'rebase', onto: r.short }) });
      out.push({ id: `irebase:${r.name}`, label: `Rebase onto ${r.short}, interactively`, group: 'Branch', run: () => info && void rebase.load(info.path, r.short) });
    }
    // Every tag, not the first few hundred. The palette scores the whole list and shows the
    // best sixty, so a capped list is not a shorter list — it is a tag that cannot be found at
    // all, and the kernel carries five thousand of them.
    for (const r of refs.groups.tags) {
      out.push({ id: `co:${r.name}`, label: `Checkout tag ${r.short}`, group: 'Tag', run: () => void act({ kind: 'checkout', rev: r.short }) });
    }
    return out;
  });

  /** The pill being dragged and the one under it, so both can be marked. */
  let dragged = $state<string | null>(null);
  let dragOver = $state<string | null>(null);

  function startPillDrag(event: DragEvent, short: string) {
    dragged = short;
    // The name as text too, so a drop into another application gets something useful.
    event.dataTransfer?.setData('text/plain', short);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
  }

  function endPillDrag() {
    dragged = null;
    dragOver = null;
  }

  function overPill(event: DragEvent, short: string) {
    if (dragged === null || dragged === short) return;
    // Preventing the default is what marks this a valid drop target.
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
    dragOver = short;
  }

  function dropOnPill(event: DragEvent, short: string) {
    event.preventDefault();
    const source = dragged;
    endPillDrag();
    if (source !== null && source !== short) void dropRef(source, short);
  }

  /** Whether `name` is a tracking branch, which is what decides a drop's direction. */
  function isRemoteRef(name: string): boolean {
    return refs.groups.remote.some((r) => r.short === name);
  }

  /** The branch part of a tracking name, so `origin/main` and `main` can be compared. */
  function withoutRemote(name: string): string {
    const at = name.indexOf('/');
    return at < 0 ? name : name.slice(at + 1);
  }

  /**
   * A branch dropped onto another.
   *
   * The reference reads the gesture as "bring `source` into `target`", which needs `target`
   * checked out first — dropping onto a branch you are not on otherwise merges into the wrong
   * one silently. A branch and its own tracking branch are the exception: there the gesture is
   * a transfer, and which way round it is decides whether that is a push or a pull.
   */
  async function dropRef(source: string, target: string) {
    // The same pair of names in the opposite order, which is the whole of the gesture.
    const sameBranch = withoutRemote(source) === withoutRemote(target);
    if (sameBranch && !isRemoteRef(source) && isRemoteRef(target)) {
      await act({ kind: 'push', remote: null, setUpstream: true, refspec: null, tags: false, forceWithLease: false, delete: false });
      return;
    }
    if (sameBranch && isRemoteRef(source) && !isRemoteRef(target)) {
      if (target !== headName) await act({ kind: 'checkout', rev: target });
      await act({ kind: 'pull', remote: null, mode: 'ffOnly' });
      return;
    }

    // Opening a request is only offered where there is a host signed in to talk to, and only
    // for two local branches: a tracking branch is already on the host.
    const canPropose =
      hosting.available && !isRemoteRef(source) && !isRemoteRef(target);
    const choices = [
      { id: 'merge', label: 'Merge', primary: true },
      { id: 'rebase', label: 'Rebase' },
      ...(canPropose ? [{ id: 'propose', label: requestWord }] : []),
    ];

    const { choice } = await ask({
      title: `Bring ${source} into ${target}?`,
      detail:
        target === headName
          ? ''
          : `${target} will be checked out first, since that is the branch the work lands on.`,
      asksText: false,
      placeholder: '',
      initial: '',
      choices,
    });
    if (choice === null) return;
    if (choice === 'propose') {
      proposing = source;
      return;
    }
    if (target !== headName) await act({ kind: 'checkout', rev: target });
    if (choice === 'rebase') await act({ kind: 'rebase', onto: source });
    else await act({ kind: 'merge', rev: source, mode: 'auto' });
  }

  /** The toolbar's seven buttons, each the commonest form of its action. */
  function toolbarAction(name: string) {
    const branch = headName;
    switch (name) {
      case 'patch': return void patchButton();
      case 'undo': return void act({ kind: 'undo' });
      case 'redo': return void act({ kind: 'redo' });
      case 'fetch': return void act({ kind: 'fetch', remote: null });
      case 'pull': return void act({ kind: 'pull', remote: null, mode: 'ffOnly' });
      // set-upstream on every push: it is a no-op once one is configured, and without it the
      // first push of a new branch fails with advice instead of pushing.
      case 'push': return void act({ kind: 'push', remote: null, setUpstream: true, refspec: null, tags: false, forceWithLease: false, delete: false });
      case 'stash': return void act({ kind: 'stashPush', message: null });
      case 'pop': return void act({ kind: 'stashApply', index: 0, pop: true });
      case 'terminal': return terminal.toggle();
      case 'branch': {
        void (async () => {
          const { choice, text } = await ask({
            title: 'New branch',
            detail: `Created at ${branch ?? 'HEAD'} and checked out.`,
            asksText: true,
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
   * Walks the repository again after the set of tips changed, and replaces everything placed
   * on it.
   *
   * The same sequence a ref move takes and in the same order: the walk first, then the refs
   * and stashes that are resolved to rows against it. `forget` is the one addition — the graph
   * skips its fast first paint when it is reopening the repository already on screen, which is
   * right for a reload and wrong here, where the rows are about to mean a different set of
   * commits.
   */
  async function rewalkForScope(path: string) {
    graph.forget();
    await graph.open(path);
    await refs.load(path);
    await stashes.load(path);
  }

  /** Takes a branch or tag out of the walk, or puts it back. */
  async function toggleHidden(ref: PlacedRef) {
    if (!info) return;
    const path = info.path;
    await scope.toggleHidden(path, ref.name);
    await rewalkForScope(path);
  }

  /** Walks only this ref, or leaves solo when it is the one already soloed. */
  async function soloRef(ref: PlacedRef | null) {
    if (!info) return;
    const path = info.path;
    await scope.setSolo(path, ref?.name ?? null);
    await rewalkForScope(path);
  }

  /** Back to the whole graph, from the banner. */
  async function showEverything() {
    if (!info) return;
    const path = info.path;
    await scope.showEverything(path);
    await rewalkForScope(path);
  }

  /**
   * Goes to a ref in the graph, widening the view first when it is not in it.
   *
   * Without this a soloed repository has a sidebar of rows that do nothing, since every branch
   * but one is outside the walk and has no row to scroll to. Clicking one is a clear enough
   * statement of wanting to see it.
   */
  async function selectRef(ref: PlacedRef) {
    if (ref.row !== null) {
      await reveal(ref.row);
      return;
    }
    if (!info || scope.walks(ref.name)) return;
    const path = info.path;
    await scope.reveal(path, ref.name);
    await rewalkForScope(path);
    const again = refs.all.find((r) => r.name === ref.name);
    if (again?.row != null) await reveal(again.row);
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
      // Deliberately not awaited. The engine holds the scope and walks by it, so a repository
      // left soloed is already narrowed by the time the rows arrive; this is only what the
      // panel draws with — which row carries the struck eye, and whether the banner is up.
      void scope.load(info.path);
      // Before the walk rather than after it. The branch list needs a row per ref, and the
      // engine answers from whatever walk it has, so this fills the panel in a moment instead
      // of leaving it saying the kernel has no branches for the six seconds the real walk
      // takes. It is asked again below, once the rows are the real ones.
      void refs.load(info.path);
      await graph.open(info.path);
      await refs.load(info.path);
      await stashes.load(info.path);
      await worktree.load(info.path);
      // A repository can be opened mid-merge, so the tool has to be there on arrival rather
      // than only after an action of ours stopped.
      await merge.load(info.path);
      void remotes.load(info.path);
      // Deliberately not awaited: a host that is slow or unreachable must not hold up the
      // window, and the section simply appears when the answer arrives.
      void hosting.load(info.path);
    } catch (e) {
      error = messageOf(e);
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
    showActivity = false;
    // Everything the panels show belongs to the repository being left. Left up, it is another
    // repository's branches and another repository's changes under the new one's name — and it
    // is what the loading screen exists to replace.
    stashes.clear();
    refs.clear();
    scope.clear();
    worktree.clear();
    showSubmodule = null;
    submoduleAt = null;
    showWip = false;
    scrollTop = 0;
    if (scroller) scroller.scrollTop = 0;
    // The lane column's fitted width belongs to the graph it was fitted to.
    panes.refit();
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
    // `.` means nothing was asked for. Opening it would open whatever directory the process
    // happens to have been launched from, which on a packaged application is somebody's home
    // or the root of the disk; the start page is the honest answer to "nothing yet".
    if (requested !== '.') await tabs.open(requested);
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
    if (info.head.kind === 'detached') {
      // No ref names it, so the row has to be looked up by object id. Picking some branch
      // instead — which is what this used to do — moved the view to a commit that was not the
      // one just checked out.
      const row = await graphRowOf(info.path, info.head.oid).catch(() => null);
      if (row !== null) await reveal(row);
      return;
    }
    const name = info.head.name;
    const target = refs.groups.local.find((r) => r.short === name);
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
    const wasHead = headMark;

    if (change.index || change.worktree) {
      await worktree.load(path);
      // The diff on screen is of a file in that working tree, and a diff of what a file used
      // to say is worse than no diff at all.
      await diff.reload(path);
    }
    if (change.ops) await merge.load(path);
    if (!change.refs && !change.graph) return;

    info = await open(path).catch(() => info);
    // Before the walk, and awaited, unlike on open. A ref change is how a soloed branch stops
    // existing — deleted from a terminal, or by another client — and the engine walks by the
    // name it was given. Reading the scope is what prunes a name the repository no longer has,
    // so doing it after the walk left an empty graph under a banner naming a branch that had
    // just been deleted, with the walk already done against the dead name.
    if (change.refs) await scope.load(path);
    // The walk first, then the things that are placed on it. Refs and stashes are resolved to
    // the row they sit on by the graph store, so loading them against the previous walk left
    // anything new — a stash above all, whose commit no branch reaches — with no row and a row
    // is what makes it clickable. Switching tabs appeared to fix it because that path has
    // always been in this order.
    if (change.refs || change.graph) await graph.open(path);
    await refs.load(path);
    await stashes.load(path);
    // Only when it actually moved: following HEAD on every commit would drag the view away
    // from whatever the user was reading.
    if (headMark !== wasHead) await focusHead();
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

  /**
   * The start page, which is what a new tab is.
   *
   * It used to be a directory picker, which is fine when the repository is already on the
   * machine and no help at all when it is not. Opening, cloning and creating are the three
   * ways to arrive at one, and this is where they are.
   */
  function openAnother() {
    showStart = true;
  }

  /*
   * The page reads what it shows when it appears, whether it was asked for or is simply what
   * is left when nothing is open.
   */
  $effect(() => {
    if (showStart || tabs.session.tabs.length === 0) void startPage.load();
  });

  /** Opens a repository from the start page, and puts the page away. */
  async function openFromStart(path: string) {
    showStart = false;
    await tabs.open(path);
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
      asksText: false,
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
      asksText: false,
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
   * What is known about each row on screen.
   *
   * The engine's map is keyed by object id, because a row is a position in one walk and the
   * walk is replaced whenever the repository moves. This turns it back into rows for the frame
   * in hand, which is the only frame those rows mean anything in.
   *
   * Derived rather than looked up per use, so a row's object id is built once per repaint
   * instead of once for its initials, once for its summary and once for its body.
   */
  const visibleMeta = $derived.by(() => {
    const frame = graph.frame;
    const meta = graph.meta;
    const found = new Map<number, CommitMeta>();
    if (!frame) return found;
    for (const row of rows) {
      const local = localRow(frame, row);
      if (local === null) continue;
      const entry = meta.get(oidOf(frame, local));
      if (entry) found.set(row, entry);
    }
    return found;
  });

  /**
   * Reading the metadata here rather than inside the canvas keeps the redraw reactive: the
   * identity of this function changes whenever a metadata block lands, which is the signal the
   * canvas repaints on.
   */
  const nodeInitials = $derived.by(() => {
    const meta = visibleMeta;
    return (row: number) => {
      const author = meta.get(row)?.author;
      return author === undefined ? null : initialsOf(author);
    };
  });

  /**
   * What fixes a node's fill.
   *
   * The email, not the display name: the same person commits as "Linus Torvalds" and
   * "torvalds" over a long history, and a node that changes colour partway down the graph
   * defeats the point of colouring it.
   */
  const nodeAuthor = $derived.by(() => {
    const meta = visibleMeta;
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
   * How wide the lane column has to be for the lanes on screen.
   *
   * A shipped width cannot be right for both a linear repository and a merge-heavy one, and
   * the corridor of empty pixels the wide setting left between a commit's node and its message
   * is what made the two read as unrelated.
   */
  /**
   * The metrics the canvas is drawing with, so anything placed beside a lane agrees with it.
   *
   * The canvas draws tighter than the shipped pitch once a merge region needs more lanes than
   * the column has room for, and a strip measured against the shipped pitch would then start
   * somewhere in the middle of the lanes.
   */
  const laneMetrics = $derived(
    fittedMetrics(widestLane(graph.frame, rows), panes.widths.graph),
  );

  /**
   * The empty pixels on a row between its outermost lane and the commit message.
   *
   * The column is as wide as the widest row on screen and never narrows again, so on a linear
   * stretch of a repository that has merge regions elsewhere this is most of the column: a
   * corridor of nothing between a commit's node and the text about it. The lane colour fills
   * it, which is what ties the two together.
   *
   * Measured per row rather than for the screen, because the row beside a thirty-lane merge
   * has no gap at all and painting one would cover the lanes.
   */
  function laneGap(row: number): number {
    if (graph.frame === null || localRow(graph.frame, row) === null) return 0;
    // To the centre of the outermost node, not past its edge: the band runs under the right
    // half of it, which is what ties the colour to the commit rather than leaving it floating
    // beside one. The canvas is drawn over the top, so the node stays a circle.
    const from = laneX(widestLane(graph.frame, [row]), laneMetrics);
    return Math.max(0, panes.widths.graph - from);
  }

  const laneFit = $derived.by(() => {
    if (rows.length === 0) return 0;
    const want = graphWidthFor(widestLane(graph.frame, rows), DEFAULT_METRICS);
    // Never more than a share of the pane. A merge region thirty lanes wide would otherwise
    // take the commit message with it, and a graph beside no message is not worth the trade;
    // past this the lanes are drawn tighter instead.
    return Math.min(want, Math.round(paneWidth * 0.35));
  });

  /*
   * Applied untracked, because `fitGraph` reads the width before deciding to widen it: tracked,
   * the effect would depend on the very state it writes and re-run itself until Svelte gave up,
   * which shows as a window that renders nothing at all.
   */
  $effect(() => {
    const px = laneFit;
    if (px > 0) untrack(() => panes.fitGraph(px));
  });

  /**
   * The lane a row's commit sits in, as a token number.
   *
   * The eight lane colours repeat, so this is the lane modulo eight and matches exactly what
   * the canvas drew for that row.
   */
  function laneOf(row: number): number {
    const local = localRow(graph.frame, row);
    if (local === null || !graph.frame) return 1;
    return ((graph.frame.lanes[local] ?? 0) % 8) + 1;
  }

  /**
   * How many characters a ref name has room for.
   *
   * Derived from the column the user has dragged rather than fixed: widening the column should
   * show more of the name, which is the only reason to widen it.
   */
  const refChars = $derived(Math.max(10, Math.floor((panes.widths.refs - 62) / 5.9)));

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
    // A hidden ref is not drawn at all. Its commits often stay, because a branch that is
    // walked still reaches them, and leaving the label on one of them put the name of a branch
    // the user had just hidden back on the graph — with the struck eye beside it in the panel
    // saying the opposite.
    return [...labels].filter((r) => !scope.hides(r.name)).sort((a, b) => rank(a) - rank(b));
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

</script>

<svelte:window onkeydown={onKey} onfocus={() => void refreshOnFocus()} oncontextmenu={platformMenu} />

<main>
  <header>
    <!-- The application's own mark, the same one the icon carries: one commit and the two
         branches that leave it. -->
    <svg class="logo" viewBox="0 0 512 512" width="18" height="18" aria-hidden="true">
      <g fill="none" stroke="currentColor" stroke-width="52" stroke-linecap="round">
        <path d="M256 392 L256 300" />
        <path d="M256 300 Q256 212 152 172" />
        <path d="M256 300 Q256 212 360 172" />
      </g>
      <g fill="currentColor">
        <circle cx="256" cy="396" r="52" />
        <circle cx="152" cy="164" r="52" />
        <circle cx="360" cy="164" r="52" />
      </g>
    </svg>
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
      class="chrome"
      onclick={() => openActivity()}
      title="Activity logs: what Coral has been doing"
      aria-label="Activity logs"
    ><ChromeMark kind="logs" /></button>
    <button
      class="chrome"
      onclick={() => openPreferences()}
      title="SSH keys, signing and preferences"
      aria-label="Settings"
    ><ChromeMark kind="settings" /></button>
    <!--
      Both faces are drawn and one is turned away, rather than swapped in and out. A theme
      switch that changes under the pointer with no movement reads as a redraw; turning is
      the one thing that says the button did something.
    -->
    <button
      class="chrome swap"
      class:dark={theme.current === 'dark'}
      onclick={() => theme.toggle()}
      title={theme.current === 'light' ? 'Switch to the dark theme' : 'Switch to the light theme'}
      aria-label="Switch theme"
    >
      <span class="face moon"><ChromeMark kind="moon" /></span>
      <span class="face sun"><ChromeMark kind="sun" /></span>
    </button>
  </header>

  <TabBar
    {tabs}
    newTab={showStart || tabs.session.tabs.length === 0}
    onOpen={openAnother}
    onCloseNew={() => (showStart = false)}
    onAsk={askText}
  />

  {#if info && !showStart}
    {#if views.current.toolbar}
    <Toolbar
      repo={TabsState.title(tabs.active ?? { id: 0, path: info.path, submodule: null, group: null, missing: false })}
      submodule={tabs.active?.submodule ?? null}
      branch={headName ?? 'detached'}
      busy={worktree.busy || actions.busy}
      comparing={selection.pair !== null}
      terminalOpen={terminal.open}
      onAction={toolbarAction}
      onLeaveSubmodule={() => void leaveSubmodule()}
      onPullMenu={pullMenu}
      onPushMenu={pushMenu}
    />
    {/if}
  {/if}

  {#if graph.transportWarning}
    <p class="banner">{graph.transportWarning}</p>
  {/if}
  {#if error}
    <p class="banner error">{error}</p>
  {:else if graph.error}
    <p class="banner error">{graph.error}</p>
  {/if}

  <!--
    Preferences and the log sit outside the branch that needs a loaded graph. They were inside
    it, which meant that a machine whose git is too old for Coral to open anything could not
    reach the page that exists to point Coral at a different git — nor the log that would have
    said why.
  -->
  {#if showPrefs}
    <Preferences
      {signing}
      {ssh}
      {experimental}
      hasRepository={info !== null}
      onPickGit={() => void chooseGitProgram()}
      onClose={() => (showPrefs = false)}
      onCopied={(ok, what) =>
        ok
          ? toasts.push('ok', `Copied the ${what}`)
          : toasts.push('error', `Could not copy the ${what}`)}
    />
  {/if}
  {#if showActivity}
    <Activity {activity} onClose={() => (showActivity = false)} />
  {/if}

  <!--
    Shown when asked for, and whenever there is nothing else to show. It takes the place of the
    workspace rather than covering the window, so the tab bar it was reached from is still
    there — and a window with no repository in it is no longer an empty grey rectangle that
    says nothing about what to do next.
  -->
  {#if showStart || tabs.session.tabs.length === 0}
    <Start
      start={startPage}
      onOpen={(path) => void openFromStart(path)}
      onPickDirectory={pickDirectory}
      onConfirm={confirmThat}
      onClose={tabs.session.tabs.length === 0 ? null : () => (showStart = false)}
    />
  <!--
    Anything with a repository open and no rows yet, not only the moment the walk is running:
    between the session loading and the walk starting there is a beat where none of the other
    branches matched and the body was empty, and that beat is the first thing the window shows.

    Both errors have to be excluded, not just the graph's. A repository that could not be
    opened never reaches the walk, so the graph has no error to report — and a screen that says
    "reading the repository" under a banner explaining that it could not be read is a window
    that has hung, whatever it is really doing.
  -->
  {:else if !graph.frame && graph.error === null && error === null}
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
        detachedHead={info?.head.kind === 'detached'
          ? { oid: info.head.oid, row: detachedRow }
          : null}
        stashes={stashes.list}
        submodules={refs.submodules}
        remotes={remotes.list}
        openSubmodule={tabs.active?.submodule ?? null}
        onSelect={reveal}
        onOpenSubmodule={openSubmodule}
        onRemoteMenu={remoteMenu}
        onStashMenu={stashMenu}
        onRefMenu={refMenu}
        onInitAllSubmodules={() => void initSubmodule(null, false)}
        onSubmoduleMenu={submoduleMenu}
        onDropRef={dropRef}
        pullRequests={hosting.pullRequests}
        pullRequestLabel={hosting.view?.host?.kind === 'gitlab' ? 'Merge requests' : 'Pull requests'}
        focusFilter={filterTick}
        collapsed={views.current.collapsed}
        onCollapse={(section, closed) => views.setCollapsed(section, closed)}
        onOpenPullRequest={(pr: PullRequest) => void openInBrowser(pr.webUrl)}
        scope={{ solo: scope.solo, hidden: scope.hidden }}
        onToggleHidden={(r) => void toggleHidden(r)}
        onLeaveSolo={() => void soloRef(null)}
        onShowEverything={() => void showEverything()}
        onSelectRef={(r) => void selectRef(r)}
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
    {#if proposing !== null && info}
      <NewRequest
        repo={info.path}
        source={proposing}
        targets={remoteBranchNames}
        label={requestWord}
        onClose={() => (proposing = null)}
        onOpened={(made) => {
          toasts.push('ok', `${requestWord} opened`, `#${made.number} ${made.title}`);
          void hosting.load(info?.path ?? '');
        }}
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
      <DiffView {diff} onClose={() => diff.close()} onPart={applyPart} />
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
      {#if find.open}
        <!--
          Over the list rather than above it: the bar is transient, and pushing every row down
          by its height moves the commit the reader was looking at just as they open it.
        -->
        <div class="find">
          <input
            bind:this={findField}
            value={find.query}
            oninput={(e) => info && find.type(info.path, e.currentTarget.value)}
            onkeydown={findKey}
            placeholder="Message, author or id"
            aria-label="Find a commit"
          />
          <span class="tally" aria-live="polite">
            {#if find.searching}
              searching…
            {:else if find.query.trim() === ''}
              &nbsp;
            {:else if find.matches.length === 0}
              no matches
            {:else}
              {find.at + 1} of {find.matches.length}
            {/if}
          </span>
          <button
            onclick={() => void stepFind(-1)}
            disabled={find.matches.length === 0}
            title="Previous match">↑</button
          >
          <button
            onclick={() => void stepFind(1)}
            disabled={find.matches.length === 0}
            title="Next match">↓</button
          >
          <button onclick={() => find.close()} title="Close the search">✕</button>
        </div>
      {/if}

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
            theme={theme.current}
            initials={nodeInitials}
            author={nodeAuthor}
            maxLane={widestLane(graph.frame, rows)}
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
              class:selected={selection.marks(row)}
            class:found={find.rows.has(row)}
            class:here={find.current === row}
              oncontextmenu={(e) => rightClickRow(e, row)}
            >
              <button
                class="hit"
                onclick={(e) => pick(row, e)}
                aria-label="Select commit"
              ></button>
              <!--
                One name, and a count for the rest. Two pills stacked inside a 28px row left
                each of them ten pixels tall and the name cut to fit, which is the state the
                column was in when it read as noise: a row commonly carries `master` and
                `origin/master`, and half a name twice says less than one name whole.
              -->
              <span class="cell refs">
                {#if labels.length > 1}
                  <!--
                    The marks of what is not shown, rather than only how much. A row carrying
                    `main`, `origin/main` and `github/main` is one branch in three places, and
                    a chip reading "+2" says the number and not the fact. Three marks is where
                    it stops being read at a glance, so past that it counts again.
                  -->
                  {@const rest = labels.slice(1)}
                  <button
                    class="more"
                    title={`Also here: ${rest.map((r) => r.short).join(', ')}`}
                    aria-label={`${rest.length} more ref${rest.length === 1 ? '' : 's'} on this commit: ${rest.map((r) => r.short).join(', ')}`}
                    onclick={(e) => refsMenu(e, rest)}
                  >
                    {#if rest.length <= 3}
                      {#each rest as other (other.name)}
                        {#if other.kind.kind === 'remote_branch'}
                          <HostMark kind={hostFor(other.short)} size={11} />
                        {:else}
                          <RefMark kind={other.kind.kind} />
                        {/if}
                      {/each}
                    {:else}
                      +{rest.length}
                    {/if}
                  </button>
                {/if}
                {#each labels.slice(0, 1) as label (label.name)}
                  <!--
                    A button, so the full name is reachable: hovering shows it, and a keyboard
                    can land on it and read it out. Clicking selects the row, which is what
                    clicking anywhere else on the row does.

                    Tinted with the lane its commit sits in, and right up against the lanes,
                    which is what ties the name to the node beside it. A grey pill floating at
                    the far side of the column leaves the eye to trace the line back itself.
                  -->
                  <button
                    class="pill {label.kind.kind}"
                    class:head={label.short === headName}
                    class:dragging={dragged === label.short}
                    class:over={dragOver === label.short}
                    style:--tint="var(--lane-{laneOf(row)}-soft)"
                    style:--tint-line="var(--lane-{laneOf(row)})"
                    title="{label.short}&#10;{label.name}"
                    draggable={label.kind.kind === 'local_branch' ||
                    label.kind.kind === 'remote_branch'
                      ? 'true'
                      : 'false'}
                    ondragstart={(e) => startPillDrag(e, label.short)}
                    ondragend={() => endPillDrag()}
                    ondragover={(e) => overPill(e, label.short)}
                    ondragleave={() => (dragOver === label.short ? (dragOver = null) : null)}
                    ondrop={(e) => dropOnPill(e, label.short)}
                    onclick={(e) => pick(row, e)}
                  >
                    <!-- The cap says what the ref is; for a tracking branch that is the host
                         it came from, which is more than "a branch" says. -->
                    <span class="cap">
                      {#if label.kind.kind === 'remote_branch'}
                        <HostMark kind={hostFor(label.short)} />
                      {:else}
                        <RefMark kind={label.kind.kind} />
                      {/if}
                    </span>
                    <span class="pill-text">{elideRef(label.short, refChars)}</span>
                  </button>
                {/each}
                <!--
                  Detached HEAD gets a label of its own, because nothing else on the row says
                  the commit is the one checked out and the state is easy to be in by accident.
                -->
                {#if row === detachedRow}
                  <span class="pill head detached" title="HEAD is detached at this commit">
                    <span class="cap"><RefMark kind="other" /></span>
                    <span class="pill-text">HEAD</span>
                  </span>
                {/if}
              </span>
              <span class="cell graph-col"></span>
              <span
                class="cell message"
                style:--row-tint="var(--lane-{laneOf(row)}-soft)"
                style:--row-line="var(--lane-{laneOf(row)})"
              >
                <span class="lane-strip" aria-hidden="true" style:--lane-gap="{laneGap(row)}px"
                ></span>
                <span class="summary">{visibleMeta.get(row)?.summary ?? ''}</span>
                {#if showBody}
                  <span class="detail">{flatten(visibleMeta.get(row)?.body ?? '')}</span>
                {/if}
                {#if local !== null}
                  <span class="age">{shortAge(graph.frame.times[local] ?? 0)}</span>
                  <span class="sha mono">{oidOf(graph.frame, local).slice(0, 8)}</span>
                {/if}
              </span>
            </li>
          {/each}
        </ul>
      </div>
    </div>
    <!--
      Not while a merge is stopped. The tool that settles it is the only thing worth looking at
      until it is settled, and the panel beside it can only offer a commit to select — so it
      spent half the window saying "select a commit" while the two sides being merged were
      squeezed into a column too narrow to read, with the button that takes a side clipped.
    -->
    {#if views.current.details && !merge.inProgress}
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
            commit={commitDraft}
            branch={headName}
            openPath={diff.path}
            grouping={views.current.changes}
            onGrouping={(g) => views.set('changes', g)}
            onOpenFile={openWorkingFile}
            onDiscard={(entries) => void discardChanges(entries)}
            onFileMenu={fileMenu}
          />
        </aside>
      {:else}
        <Details
          detail={selection.detail}
          repo={info?.path ?? null}
          compare={selection.pair === null
            ? null
            : {
                from: selection.pair.from.oid,
                to: selection.pair.to.oid,
                files: selection.compared,
              }}
          loading={selection.loading}
          error={selection.error}
          openPath={diff.path}
          grouping={views.current.commitFiles}
          onGrouping={(g) => views.set('commitFiles', g)}
          onOpenFile={openFile}
          onClearCompare={clearCompare}
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
        <!-- One pane per repository whose terminal has been shown, the inactive ones hidden
             rather than unmounted. A single pane showed the shell of whichever tab was open
             first; rebuilding it per tab fixed that but threw away the scrollback and the
             subscription, so coming back to a tab showed a blank pane in front of a shell
             that was still running. -->
        {#each terminalsFor as repo (repo)}
          <div class="pane" hidden={repo !== info.path}>
            <Terminal session={terminal} path={repo} onClose={() => (terminal.open = false)} />
          </div>
        {/each}
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
    asksText={question.asksText}
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
  .logo { flex: 0 0 auto; color: var(--accent); }
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
  .chrome {
    display: flex; align-items: center; justify-content: center;
    width: 26px; height: 26px; padding: 0; cursor: pointer;
    border-radius: var(--radius-1);
    border: 1px solid transparent; background: transparent; color: var(--fg-2);
  }
  .chrome:first-of-type { margin-left: auto; }
  .chrome:hover { background: var(--bg-2); border-color: var(--border); color: var(--fg-0); }

  /* The two faces occupy the same 16 pixels; only their rotation says which is showing. */
  .swap { position: relative; }
  .face {
    position: absolute; inset: 0; display: grid; place-items: center;
    transition: transform 260ms cubic-bezier(0.34, 1.3, 0.64, 1), opacity 200ms ease;
  }
  .swap .moon { transform: rotate(0deg) scale(1); opacity: 1; }
  .swap .sun { transform: rotate(-90deg) scale(0.4); opacity: 0; }
  .swap.dark .moon { transform: rotate(90deg) scale(0.4); opacity: 0; }
  .swap.dark .sun { transform: rotate(0deg) scale(1); opacity: 1; }

  @media (prefers-reduced-motion: reduce) {
    .face { transition: none; }
  }
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
  .term .pane { flex: 1; display: flex; min-width: 0; min-height: 0; }
  .term .pane[hidden] { display: none; }
  .term :global(.terminal) { flex: 1; min-width: 0; }
  /* Positioned, so the preferences screen can cover the panes without covering the
     window's own chrome. */
  .body { display: flex; flex: 1; min-height: 0; position: relative; }
  .graph { flex: 1; overflow-y: auto; position: relative; background: var(--bg-0); }
  /* Hidden rather than unmounted: remounting would refetch the frame and lose the scroll
     position every time a file is opened and closed. */
  .graph.hidden { display: none; }

  /* Column headers, matching the row grid below so the two cannot drift apart. */
  /*
   * The find bar, pinned to the top of the list it searches.
   */
  .find {
    position: sticky; top: 0; z-index: 3;
    display: flex; align-items: center; gap: var(--space-2);
    padding: var(--space-1) var(--space-2);
    background: var(--bg-1); border-bottom: 1px solid var(--border);
  }
  .find input {
    flex: 1; min-width: 0; font: inherit; font-size: 12px;
    background: var(--bg-0); color: var(--fg-0);
    border: 1px solid var(--border); border-radius: var(--radius-1);
    padding: 2px var(--space-2);
  }
  .find .tally {
    flex: 0 0 auto; color: var(--fg-2); font-size: 11px;
    font-variant-numeric: tabular-nums; min-width: 6em; text-align: right;
  }
  .find button {
    flex: 0 0 auto; font: inherit; font-size: 12px; cursor: pointer; line-height: 18px;
    background: var(--bg-0); border: 1px solid var(--border); border-radius: 3px;
    color: var(--fg-1); padding: 0 6px;
  }
  .find button:hover:not(:disabled) { background: var(--bg-2); color: var(--fg-0); }
  .find button:disabled { color: var(--fg-2); cursor: default; }

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
  /*
   * Above the rows, which is the whole reason the band can reach the nodes. The band is painted
   * by the row it belongs to, and rows are drawn after this canvas, so without the raise it
   * covered the right half of every node it ran up to. The canvas is only as wide as its own
   * column and takes no pointer events, so nothing else is behind it.
   */
  .lanes {
    position: absolute; left: calc(var(--refs-col) + var(--space-3));
    height: 0; pointer-events: none; z-index: 1;
  }
  /* The list is positioned once and the rows stack inside it in normal flow. Positioning each
     row individually put every one of them at its own computed offset; laying them out
     normally means only one element can be off, and it is snapped. */
  .rows { list-style: none; margin: 0; padding: 0; position: absolute; left: 0; right: 0; }

  .row, .wip {
    position: relative; height: var(--row-h);
    padding: 0 var(--space-3);
    /* A point above the rest of the interface. This list is what the window is for, and it is
       read at a glance down a column rather than word by word. */
    font-size: 13px; color: var(--fg-1);
    border: 0; background: none; font-family: inherit; text-align: left;
  }
  /*
   * The working copy, tinted so that uncommitted work is visible without reading the row.
   *
   * Amber rather than the accent: the accent means "this is the row you picked", and a row
   * that looks selected before anyone has clicked it is worse than no tint at all. The bar
   * down the leading edge survives a hover passing over the row below.
   */
  .wip {
    position: sticky; top: 22px; z-index: 1; cursor: pointer;
    background: var(--warn-soft); box-shadow: inset 2px 0 0 var(--warn);
    /* It is a button, and a button is shrink-to-fit even as a grid container. Every other row
       is a list item and stretches on its own, which is why the difference only showed once
       this row had a colour of its own to stop halfway across. */
    width: 100%; box-sizing: border-box;
  }
  /* The cells paint their own opaque background for antialiasing, so the tint has to be named
     on them too or the band stops halfway across the row. */
  .wip .cell.refs, .wip .cell.message { background: var(--warn-soft); }
  .wip .summary { color: var(--fg-0); font-weight: 600; }
  .wip .wip-node { border-color: var(--warn); }

  /*
   * The lane's colour sits in the gap between the node and the text, not across the message.
   *
   * It used to wash the whole message cell, and that is what made the selected commit hard to
   * find: every row was already tinted something, so the one tint that means "this is the row
   * you picked" was just another colour in a column of colours. A strip at the leading edge
   * still ties the line of text to a node three columns away, and leaves the cell itself plain
   * for the accent to land on. The reference client paints the same gap for the same reason.
   */
  .row .cell.message {
    background: var(--bg-0);
    /* The band stops at this edge and the bar starts it, so the eye is handed from the lane to
       the text rather than left to cross a gap. State replaces the colour, never the bar. */
    box-shadow: inset 2px 0 0 var(--row-line, transparent);
  }
  .row:hover .cell.message { background: var(--bg-1); }
  /*
   * The band filling that corridor. It takes no room in the row: the width and the negative
   * margin are the same number, so it is drawn entirely leftwards into the lane column and
   * stops exactly where the message cell begins, leaving that cell's own leading bar to show
   * which row is selected.
   *
   * Square, and deliberately: consecutive commits in one lane then run into a single ribbon,
   * which is the point — a branch reads as one colour from its label to its last commit.
   */
  .lane-strip {
    /* Positioned rather than laid out, so it takes no part in the row's flow and no negative
       margin has to cancel it. `right: 100%` puts its right edge exactly on the message cell's
       leading edge, which leaves that cell's own inset bar — the one that marks the selected
       row — visible instead of painted over. */
    position: absolute; right: 100%; top: 1px; bottom: 1px;
    width: var(--lane-gap, 0px);
    background: var(--row-tint, transparent);
  }
  /* Every match tinted, the one being stood on ruled as well: a screen of identical tints
     says how many matched and nothing about which one the buttons are pointing at. */
  .row.found .cell.message { background: var(--warn-soft); }
  .row.here .cell.message { box-shadow: inset 2px 0 0 var(--warn); }
  /*
   * The selected commit, tinted and given a bar down its leading edge. On a screen of rows
   * that all look alike a tint alone is easy to lose, and the bar survives a hover passing
   * over a neighbour.
   */
  .row.selected .cell.message, .wip.selected {
    background: var(--accent-soft);
    box-shadow: inset 2px 0 0 var(--accent-line);
  }
  /* Selected beats dirty: whichever row the panel on the right is showing has to be the one
     that looks picked, and the working copy is still the only amber-noded row on the list. */
  .wip.selected .cell.refs, .wip.selected .cell.message { background: var(--accent-soft); }

  .row.selected .cell.message .summary { color: var(--fg-0); font-weight: 600; }
  /*
   * Every text surface paints an opaque background of its own, and this is not optional.
   *
   * WebKit antialiases text on a composited layer with subpixel precision only where it knows
   * what is behind it; `background: none` leaves it guessing and it falls back to grayscale,
   * which reads as soft. The giveaway was that hovering a row sharpened it — the hover colour
   * was the only thing telling WebKit what the backdrop was.
   *
   * The whole page is composited, because that is what makes the wheel scroll at all.
   *
   * It has to be the cells and not the row. The lane canvas is a sibling painted before the
   * row list, so an opaque background on the row itself covers the graph completely — every
   * edge and node gone, with nothing in the console to say why.
   */
  .cell.message, .cell.refs { background: var(--bg-0); }
  .cell.message {
    /* The containing block for the lane band, which is drawn outside it to the left. */
    position: relative;
    border-radius: var(--radius-1); padding: 0 var(--space-2);
    /* The row's own height, so the highlight is a band rather than a floating pill. */
    height: 100%;
  }
  /* The whole row is the target; a button laid over it keeps that keyboard-reachable without
     nesting interactive elements inside one another. */
  .hit {
    position: absolute; inset: 0; width: 100%; height: 100%;
    background: none; border: 0; padding: 0; margin: 0; cursor: pointer;
    /*
     * Raised, because the message cell is positioned too and comes after this in the row.
     * Without it the cell sat over the overlay and a click anywhere on a commit's message did
     * nothing at all — only the avatar, which is drawn on the canvas, still selected the row.
     * The ref pills are raised above this in turn, so they keep their own clicks and titles.
     */
    z-index: 1;
  }
  /*
   * The ref column sits above the row's click overlay.
   *
   * That overlay covers the whole row, so the pointer was never over a pill and its `title`
   * never fired — a truncated branch name had no way at all to be read in full. The pills
   * select the row themselves, so raising them costs nothing.
   */
  .cell.refs { position: relative; z-index: 1; }
  .cell { min-width: 0; display: flex; align-items: center; gap: var(--space-2); }
  /*
   * Pills are pushed to the trailing edge, so each one touches the lane of the commit it
   * names. Left-aligned they floated at the far side of a column that is mostly empty, and
   * the name and its node read as two unrelated things on the same line. The cost is that the
   * left edge of the names no longer lines up; the gain is that the column stops being a gap.
   */
  .cell.refs {
    justify-content: flex-end; gap: var(--space-1);
    padding-left: var(--space-1); padding-right: 0;
    overflow: hidden;
  }
  .cell.message { gap: var(--space-3); }

  /*
   * One pill per row, tinted with the lane its commit sits in and capped with a mark saying
   * what kind of ref it is.
   *
   * The tint is the lane's own colour at low saturation rather than a colour per ref kind: on
   * a busy graph what the eye needs from this column is which line the name belongs to, and
   * the cap already says branch, tag or stash. Text stays near-black on the tint — a whole
   * pill in a saturated lane colour is unreadable at eleven pixels.
   */
  .pill {
    display: inline-flex; align-items: center; gap: 5px;
    flex: 0 1 auto; min-width: 0; font: inherit; font-size: 11px; line-height: 18px;
    padding: 0 7px 0 0; cursor: pointer;
    border-radius: 9px; border: 1px solid var(--tint-line, var(--border));
    background: var(--tint, var(--bg-1)); color: var(--fg-0);
    max-width: 100%; overflow: hidden; white-space: nowrap;
  }
  /*
   * The cap is the lane colour solid, and it is what makes the pill read as belonging to the
   * line beside it rather than as a grey chip that happens to be nearby. Its mark is drawn in
   * the pill's own background, so the colour reads as a block and not as a coloured glyph.
   */
  .cap {
    flex: 0 0 auto; display: flex; align-items: center; justify-content: center;
    align-self: stretch; width: 18px; margin-right: 1px;
    border-radius: 8px 0 0 8px;
    background: var(--tint-line, var(--fg-2)); color: var(--bg-0);
  }
  .pill-text { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .pill:hover { background: var(--bg-1); border-color: var(--tint-line, var(--border-strong)); }
  /* Dragged and dropped-on, marked the way the branch panel marks them. */
  .pill.dragging { opacity: 0.5; }
  .pill.over { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent); }
  /*
   * The branch you are on, which is the one fact this column exists to show. Filled in the
   * accent rather than in its lane: it has to be findable in one look down a screen of rows
   * that are all tinted something.
   */
  .pill.head {
    background: var(--accent); border-color: var(--accent);
    color: var(--accent-fg); font-weight: 600;
  }
  .pill.head .cap { background: var(--accent-fg); color: var(--accent); }
  .pill.head:hover { background: var(--accent); }
  /* A statement rather than a control: everything it could do, the row already does. */
  .pill.detached { cursor: default; }
  /* A control, not a label: the count opens the refs it stands for. */
  .more {
    display: inline-flex; align-items: center; gap: 3px;
    color: var(--fg-2); background: none; padding: 1px 4px;
    border: 1px dashed var(--border); border-radius: 7px;
    flex: 0 0 auto; font: inherit; font-size: 10px; line-height: 13px; cursor: pointer;
    position: relative; z-index: 1;
  }
  .more:hover { color: var(--fg-0); border-color: var(--border-strong); }

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
  /* Dimmer than the summary it continues, but not the dimmest thing on the row: at `fg-2` the
     half of the line that carries the body read as grey filler beside the date and the id. */
  .detail {
    flex: 1 1 0; min-width: 0; color: var(--fg-1);
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
    width: var(--details-w, 340px); flex: 0 0 auto; min-height: 0;
    border-left: 1px solid var(--border); background: var(--bg-1);
    padding: var(--space-3);
    /* The panel itself does not scroll: the two file lists inside it do, each with half the
       height, so the commit box stays where it is and both lists are always visible. */
    display: flex; flex-direction: column; overflow: hidden;
  }
</style>
