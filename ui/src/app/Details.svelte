<script lang="ts">
  import FileTree from './FileTree.svelte';
  import Icon from './Icon.svelte';
  import { shortAge } from './age';
  import { copyText } from './clipboard';
  import { authorColourIndex, initialsOf } from '../graph/initials';
  import { buildTree } from '../diff/tree';
  import { commitTree } from '../ipc/commands';
  import { messageOf } from '../ipc/error';
  import type { Entry } from './FileTree.svelte';
  import type { ChangedFile, CommitDetail, FileChange } from '../ipc/types';
  import type { Grouping } from '../state/views.svelte';

  const {
    detail,
    repo,
    compare,
    nothing,
    loading,
    error,
    openPath,
    grouping,
    onGrouping,
    onOpenFile,
    onClearCompare,
    onCopied,
  }: {
    detail: CommitDetail | null;
    /** Which repository the commit is in, for reading its tree. */
    repo: string | null;
    /**
     * The two commits being compared and what differs between them, when a second commit has
     * been picked. Takes the place of the one commit's own details.
     */
    compare: { from: string; to: string; files: ChangedFile[] } | null;
    /** Goes back to the newer of the two on its own. */
    onClearCompare: () => void;
    /** True when the graph holds no commits, so there is nothing to invite a click on. */
    nothing: boolean;
    loading: boolean;
    error: string | null;
    /** Path whose diff is on screen, so the list can mark it. */
    openPath: string | null;
    /** How the file list is arranged. Remembered by the window, not by this component. */
    grouping: Grouping;
    onGrouping: (grouping: Grouping) => void;
    onOpenFile: (path: string) => void;
    /** Says whether the clipboard took the object id, which a button cannot tell on its own. */
    onCopied: (ok: boolean, what: string) => void;
  } = $props();

  /**
   * The disc behind an author's initials, keyed by who they are.
   *
   * The same eight fills the canvas draws its nodes with, chosen the same way, so the row that
   * was clicked and the panel that answers carry one mark between them. They are dark in both
   * themes on purpose — the letters on them are white in both.
   */
  const NODE_FILLS = ['--node-1', '--node-2', '--node-3', '--node-4',
                      '--node-5', '--node-6', '--node-7', '--node-8'];

  function faceOf(name: string): string {
    return `var(${NODE_FILLS[authorColourIndex(name, NODE_FILLS.length)] ?? '--node-1'})`;
  }

  async function onCopyOid(oid: string): Promise<void> {
    onCopied(await copyText(oid), 'The object id');
  }

  function absolute(seconds: number): string {
    return new Date(seconds * 1000).toLocaleString();
  }

  /** A commit that was rebased or cherry-picked has two different answers. */
  const rewritten = $derived(
    detail !== null &&
      (detail.commit.author.name !== detail.commit.committer.name ||
        detail.commit.author.time !== detail.commit.committer.time),
  );

  /**
   * Whether to list the whole repository at this commit rather than what it changed.
   *
   * The box used to lift a five-hundred-file cap on the list of changed files, which is a
   * thing almost no commit reaches: ticking it did nothing anyone could see. What it is for
   * is the other question a commit raises — not "what did this change" but "what was here" —
   * and that is a tree, so ticking it shows one.
   */
  let showAll = $state(false);

  /** How many files are listed before the rest are summarised. */
  const LIMIT = 500;

  const files = $derived(compare?.files ?? detail?.files ?? []);
  const shown = $derived(files.slice(0, LIMIT));

  /** The paths of everything at this commit, once asked for. */
  let everything = $state<string[]>([]);
  let loadingAll = $state(false);
  let allError = $state<string | null>(null);

  const changed = $derived(new Set(files.map((f) => f.path)));

  /**
   * The whole tree as the file list draws it: every path, marked where this commit touched it.
   *
   * A path the commit did not touch has no change of its own, so it is drawn without a mark
   * and opens as the file was at this commit rather than as a diff.
   */
  const everythingAsFiles = $derived(
    everything.map((path) => ({
      path,
      oldPath: null,
      change: (changed.has(path) ? files.find((f) => f.path === path)?.change : null) ?? null,
    })),
  );

  const tree = $derived(
    buildTree<Entry>(
      showAll ? everythingAsFiles : shown.map((f) => ({ ...f, change: f.change as FileChange })),
    ),
  );

  // Read when the box is ticked, and again whenever the commit changes under it.
  $effect(() => {
    const rev = compare?.to ?? detail?.commit.oid ?? null;
    if (!showAll || rev === null || repo === null) {
      everything = [];
      allError = null;
      return;
    }
    loadingAll = true;
    allError = null;
    commitTree(repo, rev)
      .then((paths) => {
        everything = paths;
      })
      .catch((e: unknown) => {
        allError = messageOf(e);
        everything = [];
      })
      .finally(() => {
        loadingAll = false;
      });
  });

  /** In tree mode the directory is a heading and only the file name repeats. */
  function split(path: string): { dir: string; name: string } {
    const at = path.lastIndexOf('/');
    return at < 0
      ? { dir: '', name: path }
      : { dir: path.slice(0, at + 1), name: path.slice(at + 1) };
  }

  const mark: Record<string, string> = {
    added: 'A',
    deleted: 'D',
    modified: 'M',
    renamed: 'R',
    copied: 'C',
  };
</script>

<aside>
  {#if error}
    <p class="error">{error}</p>
  {:else if loading}
    <p class="muted">Loading…</p>
  {:else if compare}
    <!--
      Two commits, so there is no one message or author to show: what the panel has to say is
      what differs between them, and which two they are.
    -->
    <h2>Comparing two commits</h2>
    <dl class="fields">
      <dt>From</dt>
      <dd class="mono">{compare.from.slice(0, 12)}</dd>
      <dt>To</dt>
      <dd class="mono">{compare.to.slice(0, 12)}</dd>
    </dl>
    <button class="single" onclick={onClearCompare}>Show just the newer commit</button>
  {:else if !detail}
    <p class="muted">
      {#if nothing}
        A commit's author, message and files appear here.
      {:else}
        Select a commit, or hold Ctrl and pick a second one to compare.
      {/if}
    </p>
  {:else}
    <h2>{detail.commit.summary}</h2>

    <!--
      Who wrote it, on one line, with the same initials disc the graph draws its nodes with —
      so the row you clicked and the panel that answers wear the same mark.
    -->
    <div class="who">
      <span class="face" style:background={faceOf(detail.commit.author.name)}>
        {initialsOf(detail.commit.author.name)}
      </span>
      <span class="names">
        <span class="name">{detail.commit.author.name}</span>
        <span class="mail">{detail.commit.author.email}</span>
      </span>
      <span class="when" title={absolute(detail.commit.author.time)}>
        {shortAge(detail.commit.author.time)}
      </span>
    </div>

    {#if detail.commit.body}
      <pre class="body">{detail.commit.body}</pre>
    {/if}

    <!--
      Exactly one dd per dt. The grid places items in order, so a second dd for the same term
      flows back into the label column and its content — an unbreakable object id — sets that
      column's width, leaving nothing for the values.
    -->
    <dl>
      {#if rewritten}
        <dt>Committer</dt>
        <dd>
          {detail.commit.committer.name} &lt;{detail.commit.committer.email}&gt;
          <span class="when">{absolute(detail.commit.committer.time)}</span>
        </dd>
      {/if}
      <dt>Commit</dt>
      <dd class="mono break">
        <span class="oid">{detail.commit.oid}</span>
        <button class="copy" title="Copy the object id" onclick={() => onCopyOid(detail.commit.oid)}>
          <Icon name="copy" size={12} />
        </button>
      </dd>
      {#if detail.commit.parents.length > 0}
        <dt>{detail.commit.parents.length > 1 ? 'Parents' : 'Parent'}</dt>
        <dd class="mono">
          {#each detail.commit.parents as parent (parent)}
            <span class="parent">{parent.slice(0, 12)}</span>
          {/each}
        </dd>
      {/if}
    </dl>
  {/if}

  {#if compare || detail}
    <h3>
      {files.length} file{files.length === 1 ? '' : 's'}{compare ? ' differ' : ''}
    </h3>
    <div class="filebar">
      <!-- Path and Tree are how the changed files are arranged. Everything at this commit is
           thousands of files in a directory structure, which only reads as a tree, so the
           choice does not apply while that is what is on screen. -->
      <div class="toggle" class:hidden={showAll}>
        <button class:on={grouping === 'path'} onclick={() => onGrouping('path')}>Path</button>
        <button class:on={grouping === 'tree'} onclick={() => onGrouping('tree')}>Tree</button>
      </div>
      <label class="all">
        <input type="checkbox" bind:checked={showAll} />
        View all files
      </label>
    </div>
    {#if showAll}
      {#if loadingAll}
        <p class="muted">Reading the tree…</p>
      {:else if allError}
        <p class="error">{allError}</p>
      {:else}
        <p class="muted count">{everything.length} files at this commit</p>
        <FileTree nodes={tree} {openPath} {onOpenFile} startClosed />
      {/if}
    {:else if grouping === 'tree'}
      <FileTree nodes={tree} {openPath} {onOpenFile} />
    {:else}
    <ul class="files">
      {#each shown as file (file.path)}
        <li>
          <button
            class="file"
            class:open={file.path === openPath}
            onclick={() => onOpenFile(file.path)}
            title={file.oldPath ? `${file.path}\nfrom ${file.oldPath}` : file.path}
          >
            <span class="mark {file.change}">{mark[file.change] ?? '?'}</span>
            <span class="path">
              <span class="dir">{split(file.path).dir}</span
              ><span class="name">{split(file.path).name}</span>
            </span>
          </button>
        </li>
      {/each}
      {#if files.length > LIMIT}
        <li class="muted">…and {files.length - LIMIT} more</li>
      {/if}
    </ul>
    {/if}
  {/if}
</aside>

<style>
  aside {
    width: var(--details-w, 340px); flex: 0 0 auto; overflow-y: auto;
    border-left: 1px solid var(--border); background: var(--bg-1);
    padding: var(--space-3); font-size: var(--text-base);
  }
  /*
   * Every element that carries text in this panel paints its own opaque background.
   *
   * The panel's own `background` is not enough. It scrolls, and WebKit paints a scrolling
   * container's background into one layer and its contents into another; the contents layer is
   * transparent, so text on it drops from subpixel to grayscale antialiasing and the whole
   * column reads soft. Naming the colour on the text elements themselves is what fixes it.
   */
  h2, h3, dt, dd, .muted, .error, .all, .name, .mail, .when { background: var(--bg-1); }
  h2 {
    font-size: var(--text-lg); font-weight: 600; line-height: 1.3;
    margin: 0 0 var(--space-3); color: var(--fg-0);
    /* Balanced, so a two-line summary does not leave one word alone on the second line. */
    text-wrap: balance;
  }

  /*
   * Who wrote it, as a line rather than as a row of a table.
   *
   * The author used to be a value in the same grid as the object id, under a 10px uppercase
   * label — the least interesting kind of typography for the most human fact on the panel.
   */
  .who {
    display: flex; align-items: center; gap: var(--space-2);
    margin-bottom: var(--space-3); min-width: 0;
  }
  /* The same disc the canvas fills a node with, at the same size, so the row that was clicked
     and the panel that answers wear one mark between them. */
  .face {
    flex: 0 0 auto; width: 22px; height: 22px; border-radius: 50%;
    display: flex; align-items: center; justify-content: center;
    font-size: var(--text-mark); font-weight: 700; letter-spacing: 0.02em;
    /* White in both themes, which is why the node palette is dark in both. */
    color: #ffffff;
  }
  .names { display: flex; flex-direction: column; min-width: 0; line-height: 1.25; }
  .name {
    font-size: var(--text-md); font-weight: 600; color: var(--fg-0);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .mail {
    font-size: var(--text-sm); color: var(--fg-2);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  /* The relative date, with the exact one on the tooltip: "3 hours ago" answers the question
     people actually ask of a commit, and the timestamp answers the one they ask afterwards. */
  .when {
    margin-left: auto; flex: 0 0 auto; align-self: flex-start;
    color: var(--fg-2); font-size: var(--text-sm); font-variant-numeric: tabular-nums;
  }
  /*
   * Section headings are set as a rule with a label on it, so the panel reads as a few short
   * sections rather than one long column of similar-looking lines.
   */
  h3 {
    display: flex; align-items: center; gap: var(--space-2);
    font-size: var(--text-base); font-weight: 600;
    color: var(--fg-2); margin: var(--space-4) 0 var(--space-2);
  }
  h3::after {
    content: ''; flex: 1; height: 1px; background: var(--border);
  }
  /* Going back to one commit, which is a step out of a mode rather than an action on the
     repository, so it is set as a link rather than as a button with a fill. */
  .single {
    align-self: flex-start; font: inherit; font-size: var(--text-base); cursor: pointer;
    background: none; border: 0; padding: 0; color: var(--accent);
  }
  .single:hover { text-decoration: underline; }
  .body {
    margin: 0 0 var(--space-3); padding: var(--space-2) var(--space-3);
    /* The page, and the strongest text on it. A grey message on a grey plate is the body of
       the commit set as though it were a caption on it. */
    background: var(--bg-0); border-radius: var(--radius-1);
    /* A rule down the leading edge, as a quoted message is set everywhere else. */
    box-shadow: inset 2px 0 0 var(--border-strong);
    font-family: var(--font-mono); font-variant-ligatures: none; font-size: var(--text-sm);
    line-height: var(--leading-body);
    white-space: pre-wrap; word-break: break-word; color: var(--fg-0);
  }
  dl {
    display: grid;
    /* Capped rather than max-content: the label column must never be able to squeeze the
       values out, however long a term the panel is asked to show. */
    grid-template-columns: fit-content(35%) minmax(0, 1fr);
    gap: var(--space-2) var(--space-3); margin: 0;
  }
  dt { color: var(--fg-2); font-size: var(--text-sm); padding-top: 1px; }
  /* An address or an object id has no space to break at, so it would otherwise run past the
     panel edge and be clipped rather than wrapping. */
  dd { margin: 0; min-width: 0; color: var(--fg-1); overflow-wrap: anywhere; }
  dd.break { word-break: break-all; }
  /* The forty characters and the button that takes them sit on one line, so the id can wrap
     without the button wrapping under it. */
  dd.break { display: flex; align-items: flex-start; gap: var(--space-1); }
  .oid { min-width: 0; }
  .copy {
    flex: 0 0 auto; display: flex; padding: 2px; margin-top: -1px; cursor: pointer;
    background: var(--bg-1); border: 0; border-radius: var(--radius-1); color: var(--fg-2);
    transition: color var(--fast) var(--ease), background var(--fast) var(--ease);
  }
  .copy:hover { color: var(--fg-0); background: var(--bg-2); }
  .parent { display: block; }
  .muted { color: var(--fg-2); }
  .error { color: var(--danger); }
  .toggle.hidden { visibility: hidden; }
  .count { padding-bottom: 0; }
  .filebar {
    display: flex; align-items: center; gap: var(--space-3);
    margin-bottom: var(--space-2);
  }
  .toggle {
    display: flex; border: 1px solid var(--border); border-radius: var(--radius-1);
    overflow: hidden;
  }
  .toggle button {
    font: inherit; font-size: var(--text-sm); cursor: pointer; padding: 1px var(--space-2);
    background: var(--bg-0); border: 0; color: var(--fg-2);
  }
  .toggle button:hover { color: var(--fg-0); }
  .toggle button.on { background: var(--accent); color: var(--accent-fg); font-weight: 600; }
  .all { display: flex; align-items: center; gap: var(--space-1); font-size: var(--text-sm); color: var(--fg-2); }
  /* The directory gives way first; a truncated path that has lost its file name identifies
     nothing, which is the one part that must always survive. */
  .dir { flex: 0 1 auto; min-width: 0; color: var(--fg-2);
         overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* Same weight and colour as the row it sits on. The name used to be set in the strongest
     foreground while the directory before it was the dimmest, which at this size read as a
     bold word on the end of every path. */
  .name { flex: 0 0 auto; }
  .files { list-style: none; margin: 0; padding: 0; }
  .files li { display: flex; }
  /* Each file is the target for its own diff, so the whole row lights up rather than the
     path text alone. */
  .file {
    display: flex; align-items: center; gap: var(--space-2);
    width: 100%; text-align: left; font: inherit; font-size: var(--text-base);
    padding: 2px var(--space-2); margin: 0 calc(-1 * var(--space-2));
    background: var(--bg-1); border: 0; border-radius: var(--radius-1); cursor: pointer;
    color: var(--fg-1); overflow: hidden;
  }
  .file:hover { background: var(--bg-2); }
  .file.open {
    background: var(--accent-soft); box-shadow: inset 2px 0 0 var(--accent-line);
  }
  .path { flex: 1; min-width: 0; display: flex; overflow: hidden; white-space: nowrap; }
  .from { color: var(--fg-2); font-size: var(--text-sm); }
  /*
   * The change letter on a tinted square. A bare coloured letter at 11px is a smudge; the
   * plate gives it an edge and makes the column scannable.
   */
  .mark {
    flex: 0 0 auto; width: 15px; height: 15px; line-height: 15px; text-align: center;
    font-family: var(--font-mono); font-variant-ligatures: none; font-size: var(--text-xs);
    border-radius: 3px; background: var(--bg-2); color: var(--fg-2);
  }
  .mark.added { color: var(--ok); background: var(--ok-soft); }
  .mark.deleted { color: var(--danger); background: var(--danger-soft); }
  .mark.modified { color: var(--lane-1); }
  .mark.renamed, .mark.copied { color: var(--lane-5); }
</style>
