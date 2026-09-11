<script lang="ts">
  import Icon from './Icon.svelte';
  import Mark from './Mark.svelte';
  import type { StartState } from '../state/start.svelte';
  import type { SshKey } from '../ipc/types';
  import type { CloneHistory } from '../ipc/start';
  import { elidePath } from './path';

  const {
    start,
    sshKeys,
    defaultSshKey,
    onOpen,
    onPickDirectory,
    onClose,
    onConfirm,
  }: {
    start: StartState;
    /** Every key pair on this machine, for the clone form to offer. */
    sshKeys: SshKey[];
    /** The current profile's key, which is what the form starts on. */
    defaultSshKey: string;
    /** Asks before something that cannot be undone. Returns whether to go ahead. */
    onConfirm: (title: string, detail: string) => Promise<boolean>;
    /** Opens a repository at a path, in a tab. */
    onOpen: (path: string) => void;
    /** Asks for a directory, since the file picker belongs to the window. */
    onPickDirectory: (title: string) => Promise<string | null>;
    /** Null when there is nothing to go back to, and the page is all there is. */
    onClose: (() => void) | null;
  } = $props();

  /** Emptying the list cannot be undone, so it is asked about first. */
  async function clearAll() {
    const yes = await onConfirm(
      'Clear the recent repositories?',
      `${start.recents.length} entries are removed from this list. The repositories themselves ` +
        'are not touched.',
    );
    if (yes) await start.forgetAll();
  }

  let cloneUrl = $state('');
  let cloneParent = $state('');
  let cloneName = $state('');
  // svelte-ignore state_referenced_locally
  let cloneKey = $state(defaultSshKey);
  let cloneHistory = $state<CloneHistory>('full');
  let cloneDepth = $state(1);

  /**
   * Whether this URL will be reached over ssh.
   *
   * An https clone authenticates through the credential helper and never consults a key, so
   * offering one there is a control that does nothing. git's own two spellings are a scheme
   * and the scp-like `user@host:path`, and nothing else is ssh.
   */
  const overSsh = $derived.by(() => {
    const url = cloneUrl.trim();
    if (/^(?:https?|file|git):\/\//u.test(url)) return false;
    return url.startsWith('ssh://') || /^[^/]+@[^/]+:/u.test(url);
  });

  let createParent = $state('');
  let createName = $state('');
  let createBranch = $state('main');
  let createLfs = $state(false);

  /** The name the clone will land under, so the destination is never a surprise. */
  const clonedAs = $derived(cloneName.trim() || nameFromUrl(cloneUrl));

  /**
   * The directory name git would choose for a URL.
   *
   * The engine decides this for real; this is only so the form can say where the repository
   * will be before it is fetched.
   */
  function nameFromUrl(url: string): string {
    const trimmed = url.trim().replace(/\/+$/u, '');
    const last = trimmed.split(/[/:]/u).filter(Boolean).at(-1) ?? '';
    return last.replace(/\.git$/u, '');
  }

  async function pickInto(which: 'clone' | 'create') {
    const chosen = await onPickDirectory('Choose where it goes');
    if (chosen === null) return;
    if (which === 'clone') cloneParent = chosen;
    else createParent = chosen;
  }

  async function openOne() {
    const chosen = await onPickDirectory('Choose a repository');
    if (chosen !== null) onOpen(chosen);
  }

  async function doClone() {
    const made = await start.clone({
      url: cloneUrl,
      parent: cloneParent,
      name: cloneName,
      sshKey: overSsh ? cloneKey : '',
      history: cloneHistory,
      depth: cloneDepth,
    });
    if (made !== null) onOpen(made);
  }

  async function doCreate() {
    const made = await start.create(createParent, createName, createBranch, createLfs);
    if (made !== null) onOpen(made);
  }

  const canClone = $derived(
    cloneUrl.trim() !== '' && cloneParent !== '' && clonedAs !== '' && !start.busy,
  );
  const canCreate = $derived(createParent !== '' && createName.trim() !== '' && !start.busy);

  function when(seconds: number): string {
    return new Date(seconds * 1000).toLocaleDateString();
  }
</script>

<div class="start">
  <!--
    A column with a measure, in the middle of the window. Everything here used to be pinned to
    the left edge at its natural width, so on any real screen the page people see most often
    was a narrow strip in one corner of a large empty rectangle.
  -->
  <div class="page">
    <header>
      <!--
        The one screen that has room to say what this is. Everywhere else the mark is eighteen
        pixels in the corner of a title bar; here it can be the size of the sentence beside it.
      -->
      <Mark size={30} />
      <h2>Repositories</h2>
      {#if onClose}
        <button class="shut" title="Back" onclick={onClose}><Icon name="close" size={15} /></button>
      {/if}
    </header>

    <!--
      Three cards rather than three buttons. They are the whole purpose of this screen and they
      were the same size as the search box under them; a line each also answers the question
      that separates them, which is not obvious from three verbs alone.
    -->
    <div class="actions">
      <button class="action" onclick={() => void openOne()}>
        <span class="glyph"><Icon name="folder" size={20} /></span>
        <span class="what">Open</span>
        <span class="why">One already on this machine.</span>
      </button>
      <button
        class="action"
        class:on={start.form === 'clone'}
        onclick={() => (start.form = start.form === 'clone' ? 'none' : 'clone')}
      >
        <span class="glyph"><Icon name="pull" size={20} /></span>
        <span class="what">Clone</span>
        <span class="why">Copy one down from a host.</span>
      </button>
      <button
        class="action"
        class:on={start.form === 'create'}
        onclick={() => (start.form = start.form === 'create' ? 'none' : 'create')}
      >
        <span class="glyph"><Icon name="plus" size={20} /></span>
        <span class="what">Create</span>
        <span class="why">Start a new one here.</span>
      </button>
    </div>

    {#if start.error}
      <p class="error">{start.error}</p>
    {/if}

    <!-- Not a failure: git exited 0 and still said something worth reading, which is how a
         clone that checked nothing out announces itself. -->
    {#if start.notice}
      <p class="notice">{start.notice}</p>
    {/if}

    {#if start.form === 'clone'}
      <section class="form">
        <label>
          <span class="name">URL</span>
          <input bind:value={cloneUrl} placeholder="https://host/team/thing.git" />
        </label>
        <label>
          <span class="name">Into</span>
          <!--
            Typed as well as chosen. It was read-only, so a path on the clipboard — which is how
            most people carry one — could not be pasted, and the field looked like somewhere to
            type and silently was not. A directory that does not exist is git's to complain
            about, and the window already shows what git says.
          -->
          <input bind:value={cloneParent} placeholder="type or choose a directory" spellcheck="false" />
          <button class="pick" onclick={() => void pickInto('clone')}>Choose…</button>
        </label>
        <label>
          <span class="name">Called</span>
          <input bind:value={cloneName} placeholder={nameFromUrl(cloneUrl) || 'from the URL'} />
        </label>
        {#if overSsh}
          <!--
            Only for a URL that will actually use it. The warning is not decoration: Coral runs
            git with no terminal and no askpass, deliberately, so a key with a passphrase and
            no agent holding it fails instead of asking.
          -->
          <label class="sshkey">
            <span class="name">SSH key</span>
            <!-- An explicit handler rather than `bind:value`, as the ssh pane does it: the
                 options arrive with the answer and a binding re-selects from state after they
                 render, which is how a picker ends up showing a choice nobody made. -->
            <select value={cloneKey} onchange={(e) => (cloneKey = e.currentTarget.value)}>
              <option value="">Whatever the agent offers</option>
              {#each sshKeys as key (key.path)}
                <option value={key.path}>{key.path.split('/').pop()} · {key.comment}</option>
              {/each}
            </select>
          </label>
          <p class="note">
            The chosen key is the only one offered: <code>~/.ssh/config</code> is ignored for
            this repository, since a key named there for the same host would otherwise win. A
            key with a passphrase still has to be in your ssh agent already, because Coral
            cannot ask for one.
          </p>
        {/if}
        <!--
          Two different economies, which is why they are one choice rather than two tick
          boxes: a shallow clone cuts the history off and a partial one keeps all of it and
          leaves the file contents behind. Nobody wants to reason about both at once.
        -->
        <label class="history">
          <span class="name">Take</span>
          <select
            value={cloneHistory}
            onchange={(e) => (cloneHistory = e.currentTarget.value as CloneHistory)}
          >
            <option value="full">Everything</option>
            <option value="shallow">Recent history only</option>
            <option value="blobless">History now, file contents on demand</option>
          </select>
          {#if cloneHistory === 'shallow'}
            <input
              class="depth"
              type="number"
              min="1"
              value={cloneDepth}
              aria-label="How many commits"
              onchange={(e) => (cloneDepth = Number(e.currentTarget.value) || 1)}
            />
            <span class="units">commits</span>
          {/if}
        </label>
        {#if cloneHistory === 'shallow'}
          <p class="note">
            One branch, cut off at that many commits. Coral draws it and git can deepen it
            later with <span class="mono">git fetch --deepen</span>.
          </p>
        {:else if cloneHistory === 'blobless'}
          <p class="note">
            Every commit, no file contents until something reads one. The graph is complete and
            opening an old file needs the network. The host has to support it.
          </p>
        {/if}
        {#if cloneParent && clonedAs}
          <p class="says">It will be at <span class="mono">{cloneParent}/{clonedAs}</span></p>
        {/if}
        <div class="go">
          <button class="primary" disabled={!canClone} onclick={() => void doClone()}>
            {start.busy ? 'Cloning…' : 'Clone'}
          </button>
        </div>
      </section>
    {:else if start.form === 'create'}
      <section class="form">
        <label>
          <span class="name">In</span>
          <input bind:value={createParent} placeholder="type or choose a directory" spellcheck="false" />
          <button class="pick" onclick={() => void pickInto('create')}>Choose…</button>
        </label>
        <label>
          <span class="name">Called</span>
          <input bind:value={createName} placeholder="the repository's name" />
        </label>
        <label>
          <span class="name">First branch</span>
          <input bind:value={createBranch} placeholder="leave empty for git's default" />
        </label>
        {#if start.lfs}
          <!-- Offered only where git-lfs is installed. A tick box that fails because the program
               is not there is worse than one that is not shown. -->
          <label class="tick">
            <input type="checkbox" bind:checked={createLfs} />
            <span>Set up Large File Storage in it</span>
          </label>
        {/if}
        {#if createParent && createName.trim()}
          <p class="says">
            It will be at <span class="mono">{createParent}/{createName.trim()}</span>
          </p>
        {/if}
        <div class="go">
          <button class="primary" disabled={!canCreate} onclick={() => void doCreate()}>
            {start.busy ? 'Creating…' : 'Create'}
          </button>
        </div>
      </section>
    {/if}

    <input class="filter" placeholder="Search repositories" bind:value={start.filter} />

    <div class="recent-head">
      <h3>Recent</h3>
      {#if start.recents.length > 0}
        <!-- The list is a record of which repositories this person works on, which is not
             always something they want on the page. -->
        <button class="clear" onclick={() => void clearAll()}>Clear all</button>
      {/if}
    </div>
    {#if start.recents.length === 0}
      <p class="none">Nothing yet. Open, clone or create one and it will be listed here.</p>
    {:else if start.shown.length === 0}
      <p class="none">Nothing matches “{start.filter}”.</p>
    {:else}
      <ul class="recents">
        {#each start.shown as repo (repo.path)}
          <li>
            <button class="repo" onclick={() => onOpen(repo.path)} title={repo.path}>
              <span class="repo-name">{repo.name}</span>
              <span class="repo-path mono">{elidePath(repo.path, 72)}</span>
              <span class="repo-when">{when(repo.opened)}</span>
            </button>
            <button
              class="forget"
              title="Take {repo.name} off this list"
              onclick={() => void start.forget(repo.path)}
            ><Icon name="close" size={12} /></button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</div>

<style>
  /* Takes the place of the workspace rather than covering the window: the tab bar this page
     was reached from stays where it was. */
  .start {
    flex: 1; min-height: 0; overflow-y: auto;
    background: var(--bg-0); color: var(--fg-1);
    padding: var(--space-5) var(--space-5) var(--space-4);
    font-size: var(--text-base);
  }
  .page { max-width: 52em; margin: 0 auto; }
  header {
    display: flex; align-items: center; gap: var(--space-3); margin-bottom: var(--space-5);
    color: var(--brand);
  }
  h2 {
    flex: 1; margin: 0; font-size: var(--text-xl); font-weight: 600; color: var(--fg-0);
    background: var(--bg-0);
  }
  h3 {
    margin: var(--space-5) 0 var(--space-2); font-size: var(--text-base); font-weight: 600;
    color: var(--fg-2); background: var(--bg-0);
  }
  .shut {
    display: flex; font: inherit; line-height: 1; cursor: pointer; align-self: flex-start;
    background: var(--bg-0); border: 0; color: var(--fg-2); padding: 0 var(--space-2);
  }
  .shut:hover { color: var(--fg-0); }

  /* Three of equal width rather than three sized by their labels: they are alternatives, and
     one that is wider than the others reads as the one to press. */
  .actions {
    display: grid; grid-template-columns: repeat(3, 1fr);
    gap: var(--space-3); margin-bottom: var(--space-5);
  }
  .action {
    display: flex; flex-direction: column; align-items: flex-start; gap: var(--space-1);
    text-align: left; font: inherit; cursor: pointer;
    padding: var(--space-3) var(--space-4) var(--space-4);
    background: var(--bg-1); color: var(--fg-0);
    border: 1px solid var(--border); border-radius: var(--radius-2);
    transition: background var(--fast) var(--ease), border-color var(--fast) var(--ease);
  }
  .action:hover { background: var(--bg-2); border-color: var(--border-strong); }
  .action.on { background: var(--accent-soft); border-color: var(--accent); }
  .glyph { display: flex; color: var(--accent); margin-bottom: var(--space-1); }
  .what { font-size: var(--text-md); font-weight: 600; }
  .why { font-size: var(--text-sm); color: var(--fg-2); line-height: var(--leading-body); }

  .form {
    display: flex; flex-direction: column; gap: var(--space-3);
    max-width: 46em; margin-bottom: var(--space-4);
    padding: var(--space-4); border: 1px solid var(--border); border-radius: var(--radius-2);
    background: var(--bg-1);
  }
  .form label { display: flex; align-items: center; gap: var(--space-3); background: var(--bg-1); }
  .form .name { flex: 0 0 auto; width: 7em; color: var(--fg-2); background: var(--bg-1); }
  .form input:not([type='checkbox']) {
    flex: 1; min-width: 0; font: inherit; font-size: var(--text-base); padding: 4px var(--space-2);
    background: var(--bg-0); color: var(--fg-0);
    border: 1px solid var(--border-strong); border-radius: var(--radius-1);
  }
  .form input:focus { border-color: var(--accent); outline: none; }
  .tick { gap: var(--space-2); }
  .pick, .primary {
    flex: 0 0 auto; font: inherit; font-size: var(--text-base); cursor: pointer;
    padding: 4px var(--space-3); border-radius: var(--radius-1);
    background: var(--bg-2); border: 1px solid var(--border); color: var(--fg-1);
  }
  .pick:hover { background: var(--bg-3); color: var(--fg-0); }
  .primary { background: var(--accent); border-color: var(--accent); color: var(--accent-fg); }
  .primary:disabled { opacity: 0.5; cursor: default; }
  .go { display: flex; justify-content: flex-end; }
  .says { margin: 0; color: var(--fg-2); background: var(--bg-1); }
  /* Beneath the picker it qualifies, and quieter than the form it sits in. */
  .note { margin: -2px 0 0; font-size: var(--text-sm); color: var(--fg-2); background: var(--bg-1); }
  .history select {
    flex: 0 1 auto; min-width: 0; font: inherit; font-size: var(--text-base);
    padding: 5px var(--space-2); border-radius: var(--radius-1);
    border: 1px solid var(--border); background: var(--bg-0); color: var(--fg-0);
  }
  .depth {
    flex: 0 0 auto; width: 5em; font: inherit; font-size: var(--text-base);
    padding: 5px var(--space-2); border-radius: var(--radius-1);
    border: 1px solid var(--border); background: var(--bg-0); color: var(--fg-0);
  }
  .units { flex: 0 0 auto; color: var(--fg-2); }
  .sshkey select {
    flex: 1 1 auto; min-width: 0; font: inherit; font-size: var(--text-base);
    padding: 5px var(--space-2); border-radius: var(--radius-1);
    border: 1px solid var(--border); background: var(--bg-0); color: var(--fg-0);
  }
  .mono { font-family: var(--font-mono); font-variant-ligatures: none; }
  .error { color: var(--danger); margin: 0 0 var(--space-3); background: var(--bg-0); }
  .notice {
    color: var(--warn); margin: 0 0 var(--space-3); background: var(--bg-0);
    white-space: pre-wrap;
  }
  .none { color: var(--fg-2); margin: 0; background: var(--bg-0); }

  .filter {
    width: 100%; box-sizing: border-box; font: inherit; font-size: var(--text-base);
    padding: 5px var(--space-3);
    background: var(--bg-0); color: var(--fg-0);
    border: 1px solid var(--border-strong); border-radius: var(--radius-1);
  }
  .filter:focus { border-color: var(--accent); outline: none; }

  .recent-head { display: flex; align-items: baseline; gap: var(--space-3); }
  .clear {
    margin-left: auto; font: inherit; font-size: var(--text-sm); cursor: pointer;
    background: none; border: 0; color: var(--fg-2); padding: 0;
  }
  .clear:hover { color: var(--danger); text-decoration: underline; }
  .recents { list-style: none; margin: 0; padding: 0; }
  .recents li { display: flex; align-items: center; border-radius: var(--radius-1); }
  .recents li:hover { background: var(--bg-1); }
  /*
   * Two lines rather than three columns. Squeezed onto one line the path was the only part
   * that could give, so two checkouts of the same repository — the case the path is there to
   * tell apart — showed the same name beside the same elided middle.
   */
  .repo {
    display: grid; grid-template-columns: 1fr auto; gap: 1px var(--space-3);
    flex: 1; min-width: 0; text-align: left; font: inherit; cursor: pointer;
    padding: var(--space-2); background: none; border: 0; color: var(--fg-1);
  }
  .repo-name { grid-area: 1 / 1; color: var(--accent); font-weight: 600; font-size: var(--text-md); }
  .repo-when { grid-area: 1 / 2; color: var(--fg-2); font-size: var(--text-sm); }
  .repo-path {
    grid-area: 2 / 1 / 3 / 3; min-width: 0; color: var(--fg-2); font-size: var(--text-sm);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  /* On the row being pointed at only: a column of crosses beside a list of repositories reads
     as a list of things to delete. */
  .forget {
    flex: 0 0 auto; font: inherit; font-size: var(--text-base); line-height: 1; cursor: pointer;
    padding: 0 var(--space-2); background: none; border: 0; color: var(--fg-2);
    visibility: hidden;
  }
  .recents li:hover .forget { visibility: visible; }
  .forget:hover { color: var(--danger); }
</style>
