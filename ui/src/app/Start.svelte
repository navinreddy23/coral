<script lang="ts">
  import type { StartState } from '../state/start.svelte';
  import { elidePath } from './path';

  const { start, onOpen, onPickDirectory, onClose }: {
    start: StartState;
    /** Opens a repository at a path, in a tab. */
    onOpen: (path: string) => void;
    /** Asks for a directory, since the file picker belongs to the window. */
    onPickDirectory: (title: string) => Promise<string | null>;
    /** Null when there is nothing to go back to, and the page is all there is. */
    onClose: (() => void) | null;
  } = $props();

  let cloneUrl = $state('');
  let cloneParent = $state('');
  let cloneName = $state('');

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
    const made = await start.clone(cloneUrl, cloneParent, cloneName);
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
  <header>
    <h2>Repositories</h2>
    {#if onClose}
      <button class="shut" title="Back" onclick={onClose}>✕</button>
    {/if}
  </header>

  <div class="actions">
    <button class="action" onclick={() => void openOne()}>
      <span class="glyph" aria-hidden="true">🖿</span>Open
    </button>
    <button
      class="action"
      class:on={start.form === 'clone'}
      onclick={() => (start.form = start.form === 'clone' ? 'none' : 'clone')}
    >
      <span class="glyph" aria-hidden="true">⤓</span>Clone
    </button>
    <button
      class="action"
      class:on={start.form === 'create'}
      onclick={() => (start.form = start.form === 'create' ? 'none' : 'create')}
    >
      <span class="glyph" aria-hidden="true">＋</span>Create
    </button>
  </div>

  {#if start.error}
    <p class="error">{start.error}</p>
  {/if}

  {#if start.form === 'clone'}
    <section class="form">
      <label>
        <span class="name">URL</span>
        <input bind:value={cloneUrl} placeholder="https://host/team/thing.git" />
      </label>
      <label>
        <span class="name">Into</span>
        <input bind:value={cloneParent} placeholder="choose a directory" readonly />
        <button class="pick" onclick={() => void pickInto('clone')}>Choose…</button>
      </label>
      <label>
        <span class="name">Called</span>
        <input bind:value={cloneName} placeholder={nameFromUrl(cloneUrl) || 'from the URL'} />
      </label>
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
        <input bind:value={createParent} placeholder="choose a directory" readonly />
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

  <h3>Recent</h3>
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
          >✕</button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  /* Takes the place of the workspace rather than covering the window: the tab bar this page
     was reached from stays where it was. */
  .start {
    flex: 1; min-height: 0; overflow-y: auto;
    background: var(--bg-0); color: var(--fg-1);
    padding: var(--space-5) var(--space-5) var(--space-4);
    font-size: 12px;
  }
  header { display: flex; align-items: center; gap: var(--space-3); }
  h2 {
    flex: 1; margin: 0 0 var(--space-4); font-size: 20px; font-weight: 600; color: var(--fg-0);
    background: var(--bg-0);
  }
  h3 {
    margin: var(--space-4) 0 var(--space-2); font-size: 11px; font-weight: 700;
    text-transform: uppercase; letter-spacing: 0.07em; color: var(--fg-2);
    background: var(--bg-0);
  }
  .shut {
    font: inherit; font-size: 16px; line-height: 1; cursor: pointer; align-self: flex-start;
    background: var(--bg-0); border: 0; color: var(--fg-2); padding: 0 var(--space-2);
  }
  .shut:hover { color: var(--fg-0); }

  .actions { display: flex; gap: var(--space-3); margin-bottom: var(--space-4); }
  .action {
    display: flex; align-items: center; gap: var(--space-2);
    font: inherit; font-size: 13px; cursor: pointer;
    padding: var(--space-2) var(--space-4);
    background: var(--bg-1); color: var(--fg-0);
    border: 1px solid var(--border); border-radius: var(--radius-1);
  }
  .action:hover { background: var(--bg-2); border-color: var(--border-strong); }
  .action.on { background: var(--accent-soft); border-color: var(--accent); }
  .glyph { font-size: 14px; color: var(--accent); }

  .form {
    display: flex; flex-direction: column; gap: var(--space-3);
    max-width: 46em; margin-bottom: var(--space-4);
    padding: var(--space-4); border: 1px solid var(--border); border-radius: var(--radius-2);
    background: var(--bg-1);
  }
  .form label { display: flex; align-items: center; gap: var(--space-3); background: var(--bg-1); }
  .form .name { flex: 0 0 auto; width: 7em; color: var(--fg-2); background: var(--bg-1); }
  .form input:not([type='checkbox']) {
    flex: 1; min-width: 0; font: inherit; font-size: 12px; padding: 4px var(--space-2);
    background: var(--bg-0); color: var(--fg-0);
    border: 1px solid var(--border-strong); border-radius: var(--radius-1);
  }
  .form input:focus { border-color: var(--accent); outline: none; }
  .tick { gap: var(--space-2); }
  .pick, .primary {
    flex: 0 0 auto; font: inherit; font-size: 12px; cursor: pointer;
    padding: 4px var(--space-3); border-radius: var(--radius-1);
    background: var(--bg-2); border: 1px solid var(--border); color: var(--fg-1);
  }
  .pick:hover { background: var(--bg-3); color: var(--fg-0); }
  .primary { background: var(--accent); border-color: var(--accent); color: var(--accent-fg); }
  .primary:disabled { opacity: 0.5; cursor: default; }
  .go { display: flex; justify-content: flex-end; }
  .says { margin: 0; color: var(--fg-2); background: var(--bg-1); }
  .mono { font-family: var(--font-mono); }
  .error { color: var(--danger); margin: 0 0 var(--space-3); background: var(--bg-0); }
  .none { color: var(--fg-2); margin: 0; background: var(--bg-0); }

  .filter {
    width: 100%; max-width: 46em; box-sizing: border-box; font: inherit; font-size: 12px;
    padding: 5px var(--space-3);
    background: var(--bg-0); color: var(--fg-0);
    border: 1px solid var(--border-strong); border-radius: var(--radius-1);
  }
  .filter:focus { border-color: var(--accent); outline: none; }

  .recents { list-style: none; margin: 0; padding: 0; max-width: 60em; }
  .recents li { display: flex; align-items: center; border-radius: var(--radius-1); }
  .recents li:hover { background: var(--bg-1); }
  .repo {
    display: flex; align-items: baseline; gap: var(--space-3);
    flex: 1; min-width: 0; text-align: left; font: inherit; cursor: pointer;
    padding: 3px var(--space-2); background: none; border: 0; color: var(--fg-1);
  }
  .repo-name { flex: 0 0 auto; color: var(--accent); font-weight: 600; }
  .repo-path {
    flex: 1; min-width: 0; color: var(--fg-2); font-size: 11px;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .repo-when { flex: 0 0 auto; color: var(--fg-2); font-size: 11px; }
  /* On the row being pointed at only: a column of crosses beside a list of repositories reads
     as a list of things to delete. */
  .forget {
    flex: 0 0 auto; font: inherit; font-size: 12px; line-height: 1; cursor: pointer;
    padding: 0 var(--space-2); background: none; border: 0; color: var(--fg-2);
    visibility: hidden;
  }
  .recents li:hover .forget { visibility: visible; }
  .forget:hover { color: var(--danger); }
</style>
