<script lang="ts">
  import FileTree from './FileTree.svelte';
  import Icon from './Icon.svelte';
  import { shortAge } from './age';
  import { describe as describeSignature } from './verification';
  import LockMark from './LockMark.svelte';
  import Splitter from './Splitter.svelte';
  import { copyText } from './clipboard';
  import { forgeOf } from './forge';
  import { authorColourIndex, initialsOf } from '../graph/initials';
  import { buildTree } from '../diff/tree';
  import { commitTree, type Host, type IssueView } from '../ipc/commands';
  import { linkOf, mentioned, segments } from './reference';
  import { messageOf } from '../ipc/error';
  import type { Entry } from './FileTree.svelte';
  import type { ChangedFile, CommitDetail, FileChange, LfsLock, Verification } from '../ipc/types';
  import type { Grouping } from '../state/views.svelte';
  import {
    fitHeight,
    LIST_FLOOR_PX,
    SECTION_LIMITS,
    type SectionsState,
  } from '../state/sections.svelte';

  const {
    detail,
    verification = null,
    many = [],
    note = null,
    onEditNote,
    repo,
    compare,
    onSinceBase = null,
    onSwap = null,
    picture = null,
    nothing,
    loading,
    error,
    openPath,
    grouping,
    onGrouping,
    onOpenFile,
    onFileMenu,
    onFolderMenu,
    locks = new Map(),
    onClearCompare,
    onCopied,
    sections,
    host = null,
    tracker = null,
    readIssue = null,
    onOpenUrl = () => undefined,
    onOpenOnHost = () => undefined,
  }: {
    detail: CommitDetail | null;
    /** The commit's note, from `refs/notes/commits`; null for none or not yet read. */
    note?: string | null;
    onEditNote?: () => void;
    /** Three or more picked commits, newest first, when that is what is selected. */
    many?: { oid: string; summary: string }[];
    /** What git makes of the commit's signature; null until it has said, or for none. */
    verification?: Verification | null;
    /** Which repository the commit is in, for reading its tree. */
    repo: string | null;
    /**
     * The two commits being compared and what differs between them, when a second commit has
     * been picked. Takes the place of the one commit's own details.
     */
    compare: {
      from: string;
      /** Null for the working tree. */
      to: string | null;
      files: ChangedFile[];
      /** What the two are, said in words, for a comparison asked for by name. */
      title?: string;
      fromName?: string;
      toName?: string;
      /** Where the two parted, when measured from there. */
      base?: string | null;
      note?: string | null;
      /** What the way out of the comparison is called. */
      leave?: string;
    } | null;
    /** Measures from where the two parted, or between the tips; null where that means nothing. */
    onSinceBase?: ((on: boolean) => void) | null;
    /** Turns the comparison round; null where there is nothing to turn. */
    onSwap?: (() => void) | null;
    /** The author's picture, shown in the disc in place of the initials when there is one. */
    picture?: string | null;
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
    /** A file's own menu, for a file in the list or the tree. */
    onFileMenu?: (event: MouseEvent, path: string) => void;
    /** A folder's menu, in the tree. */
    onFolderMenu?: (event: MouseEvent, path: string) => void;
    /** Files locked on the LFS server, by path. */
    locks?: Map<string, LfsLock>;
    /** Says whether the clipboard took the object id, which a button cannot tell on its own. */
    onCopied: (ok: boolean, what: string) => void;
    /** How the panel's height is shared between the message and the files, and folds. */
    sections: SectionsState;
    /** The repository's host, for the references in a message to link to. */
    host?: Host | null;
    /** Where `ABC-123` links to, with `{key}` in it. */
    tracker?: string | null;
    /** Reads one issue, or null where there is no host to ask or nothing to ask it with. */
    readIssue?: ((number: number) => Promise<IssueView | null>) | null;
    onOpenUrl?: (url: string) => void;
    /** Shows a commit on the host's own site. Offered only where there is a `host`. */
    onOpenOnHost?: (oid: string) => void;
  } = $props();

  const closed = $derived(sections.collapsed);
  const hasFiles = $derived(compare !== null || detail !== null);
  /** A message and a file list both open, so there is a height to share between them. */
  const sharing = $derived(hasFiles && !closed.message && !closed.files);

  let asidePx = $state(0);
  let headPx = $state(0);

  /**
   * What the files must keep under a dragged message: the heading and the bar with the
   * grouping and "View all files" on it come first, and a floor that counted only a list's
   * heading left room for those and not one file.
   */
  const FILES_FLOOR_PX = LIST_FLOOR_PX + 64;

  /** The message's height as drawn: as dragged, unless the panel cannot spare it. */
  const headDrawn = $derived.by(() => {
    const want = sections.sizes.messageHeight;
    if (want === null || !sharing) return null;
    return fitHeight(want, asidePx - 32, FILES_FLOOR_PX, SECTION_LIMITS.messageHeight.min);
  });

  /** The tallest the message can be dragged here and now, which is also the most stored. */
  const headMax = $derived(
    asidePx > 0
      ? Math.max(SECTION_LIMITS.messageHeight.min, asidePx - 32 - FILES_FLOOR_PX)
      : SECTION_LIMITS.messageHeight.max,
  );

  /**
   * The disc behind an author's initials, keyed by who they are.
   *
   * The same eight fills the canvas draws its nodes with, chosen the same way, so the row that
   * was clicked and the panel that answers carry one mark between them. They are dark in both
   * themes on purpose — the letters on them are white in both.
   */
  const NODE_FILLS = ['--node-1', '--node-2', '--node-3', '--node-4',
                      '--node-5', '--node-6', '--node-7', '--node-8'];

  /**
   * Hashed from the email, exactly as the graph hashes it, or the same person came out one
   * colour in the row and another in the panel that answers it — and the whole point of the
   * disc is that the eye recognises it. The name is the fallback for a commit with no email,
   * which is what the graph falls back to too.
   */
  function faceOf(who: { name: string; email: string }): string {
    const identity = who.email.trim().toLowerCase() || who.name;
    return `var(${NODE_FILLS[authorColourIndex(identity, NODE_FILLS.length)] ?? '--node-1'})`;
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

  const signature = $derived(verification ? describeSignature(verification) : null);

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
    const rev = compare === null ? (detail?.commit.oid ?? null) : (compare.to ?? compare.from);
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

  /** The issues the message names, read from the host, a few at most and each once. */
  let mentions = $state<IssueView[]>([]);
  /** Whether the host has answered for them, so a plain link does not stand in meanwhile. */
  let mentionsRead = $state(true);
  $effect(() => {
    const commit = detail?.commit ?? null;
    const read = readIssue;
    mentions = [];
    mentionsRead = true;
    if (commit === null || read === null) return;
    const wanted = mentioned(`${commit.summary}\n${commit.body}`, host?.kind ?? null).slice(0, 4);
    if (wanted.length === 0) return;
    mentionsRead = false;
    let current = true;
    void Promise.all(wanted.map((n) => read(n))).then((found) => {
      if (!current) return;
      mentions = found.filter((f): f is IssueView => f !== null);
      mentionsRead = true;
    });
    return () => {
      current = false;
    };
  });

  /**
   * What the summary refers to that the list of issues above does not already show. The
   * summary is the button that folds the message, so it cannot hold links of its own, and it is
   * where a reference is most often written.
   */
  const summaryLinks = $derived.by(() => {
    const commit = detail?.commit ?? null;
    if (commit === null) return [];
    const out = new Map<string, string>();
    for (const part of segments(commit.summary, host?.kind ?? null, tracker)) {
      if (part.kind === 'text') continue;
      if (part.kind === 'issue' && (!mentionsRead || mentions.some((m) => m.issue.number === part.number))) continue;
      const url = linkOf(part, host, tracker);
      if (url !== null) out.set(part.text, url);
    }
    return [...out].map(([text, url]) => ({ text, url, where: URL.canParse(url) ? new URL(url).host : url }));
  });
</script>

<aside bind:clientHeight={asidePx}>
  <div
    class="head"
    class:shared={sharing}
    class:sized={headDrawn !== null}
    class:closed={hasFiles && closed.message}
    style:height={headDrawn !== null ? `${headDrawn}px` : null}
    bind:clientHeight={headPx}
  >
  {#if error}
    <p class="error">{error}</p>
  {:else if loading}
    <p class="muted">Loading…</p>
  {:else if many.length > 0}
    <!--
      A set rather than one commit or a pair: no one message or diff stands for it, so the panel
      says which commits are in it and where the things that act on all of them are.
    -->
    <h2>{many.length} commits picked</h2>
    <ul class="picked">
      {#each many as one (one.oid)}
        <li><span class="mono">{one.oid.slice(0, 8)}</span> {one.summary}</li>
      {/each}
    </ul>
    <p class="muted">
      Right-click any of them to cherry pick or revert them all. Ctrl-click one to take it out.
    </p>
  {:else if compare}
    <!--
      Two commits, so there is no one message or author to show: what the panel has to say is
      what differs between them, and which two they are.
    -->
    <h2>
      <button
        class="fold"
        aria-expanded={!closed.message}
        aria-controls="detail-message"
        onclick={() => sections.toggle('message')}
      >
        <span class="caret">
          <Icon name={closed.message ? 'chevronRight' : 'chevronDown'} size={14} />
        </span>
        <span>{compare.title ?? 'Comparing two commits'}</span>
      </button>
    </h2>
    {#if !closed.message}
    <div id="detail-message">
    <dl class="fields">
      <dt>From</dt>
      <dd>
        {#if compare.fromName && !compare.from.startsWith(compare.fromName)}<span class="end">{compare.fromName}</span>{/if}
        <span class="mono">{compare.from.slice(0, 12)}</span>
      </dd>
      <dt>To</dt>
      <dd>
        {#if compare.toName}<span class="end">{compare.toName}</span>{/if}
        {#if compare.to !== null}<span class="mono">{compare.to.slice(0, 12)}</span>{/if}
      </dd>
      {#if compare.base}
        <dt>Parted at</dt>
        <dd class="mono">{compare.base.slice(0, 12)}</dd>
      {/if}
    </dl>
    {#if compare.note}<p class="muted note">{compare.note}</p>{/if}
    {#if onSinceBase !== null}
      <label class="since">
        <input
          type="checkbox"
          checked={compare.base !== null && compare.base !== undefined}
          onchange={(e) => onSinceBase?.(e.currentTarget.checked)}
        />
        Only what {compare.toName ?? 'it'} gained since the two parted
      </label>
    {/if}
    <div class="ends">
      {#if onSwap !== null}<button class="single" onclick={() => onSwap?.()}>Swap</button>{/if}
      <button class="single" onclick={onClearCompare}>{compare.leave ?? 'Show just the newer commit'}</button>
    </div>
    </div>
    {/if}
  {:else if !detail}
    <p class="muted">
      {#if nothing}
        A commit's author, message and files appear here.
      {:else}
        Select a commit. Hold Ctrl to pick a second one to compare, or more to cherry pick or
        revert together; Shift picks a run.
      {/if}
    </p>
  {:else}
    <!--
      The message as one piece, summary over body, in a box of its own: the reference sets it
      this way, and it is what makes the panel read as a commit and then the facts about it,
      rather than as one column of lines in five sizes.

      The summary is the heading that folds the rest away: folded, it is still the line that
      says which commit the files below belong to.
    -->
    <div class="message" class:bare={closed.message || !detail.commit.body}>
      <h2>
        <button
          class="fold"
          aria-expanded={!closed.message}
          aria-controls={detail.commit.body ? 'detail-body detail-message' : 'detail-message'}
          onclick={() => sections.toggle('message')}
        >
          <span class="caret">
            <Icon name={closed.message ? 'chevronRight' : 'chevronDown'} size={14} />
          </span>
          <span>{detail.commit.summary}</span>
        </button>
      </h2>
      {#if !closed.message && detail.commit.body}
        <pre class="body" id="detail-body">{#each segments(detail.commit.body, host?.kind ?? null, tracker) as part, i (i)}{#if part.kind === 'text'}{part.text}{:else}{@const url = linkOf(part, host, tracker)}<button
              class="ref"
              title={url ?? ''}
              onclick={() => url && onOpenUrl(url)}>{part.text}</button
            >{/if}{/each}</pre>
      {/if}
    </div>

    {#if !closed.message}
    <div id="detail-message">

    <!--
      Who wrote it and when, under what they wrote, with the same initials disc the graph draws
      its nodes with — so the row you clicked and the panel that answers wear the same mark.
    -->
    <div class="who">
      <span class="face" style:background={faceOf(detail.commit.author)}>
        {#if picture !== null}
          <img src={picture} alt="" />
        {:else}
          {initialsOf(detail.commit.author.name)}
        {/if}
      </span>
      <span class="names">
        <span class="name" title={detail.commit.author.email}>{detail.commit.author.name}</span>
        <span class="authored">authored {absolute(detail.commit.author.time)}</span>
      </span>
      <span class="when" title={absolute(detail.commit.author.time)}>
        {shortAge(detail.commit.author.time)}
      </span>
    </div>

    {#if mentions.length > 0 || summaryLinks.length > 0}
      <ul class="mentions" aria-label="Issues this commit names">
        {#each mentions as m (m.issue.number)}
          <li>
            <button onclick={() => onOpenUrl(m.issue.webUrl)} title={m.issue.webUrl}>
              <span class="state {m.issue.state}">{m.issue.state}</span>
              <span class="num">{host?.kind === 'azure' ? 'AB#' : '#'}{m.issue.number}</span>
              <span class="title">{m.issue.title}</span>
            </button>
          </li>
        {/each}
        {#each summaryLinks as link (link.text)}
          <li>
            <button onclick={() => onOpenUrl(link.url)} title={link.url}>
              <span class="num">{link.text}</span>
              <span class="title muted">{link.where}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}

    {#if note !== null}
      <section class="note">
        <h3>Note</h3>
        <pre class="body">{note}</pre>
        {#if onEditNote}<button class="edit-note" onclick={onEditNote}>Edit</button>{/if}
      </section>
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
        {#if host !== null}
          <button
            class="copy"
            title={`Open on ${forgeOf(host.kind).label}`}
            onclick={() => onOpenOnHost(detail.commit.oid)}
          >
            <Icon name="external" size={12} />
          </button>
        {/if}
      </dd>
      {#if detail.commit.parents.length > 0}
        <dt>{detail.commit.parents.length > 1 ? 'Parents' : 'Parent'}</dt>
        <dd class="mono">
          {#each detail.commit.parents as parent (parent)}
            <span class="parent">{parent.slice(0, 12)}</span>
          {/each}
        </dd>
      {/if}
      {#if signature}
        <dt>Signature</dt>
        <dd class="signature {signature.tone}">
          {signature.text}
          {#if signature.why}<span class="why">{signature.why}</span>{/if}
        </dd>
      {/if}
    </dl>
    </div>
    {/if}
  {/if}
  </div>

  {#if hasFiles}
    {#if sharing}
      <Splitter
        vertical
        grip
        label="Resize the commit message and its files"
        value={sections.sizes.messageHeight ?? headPx}
        min={SECTION_LIMITS.messageHeight.min}
        max={headMax}
        onresize={(px) => sections.resize('messageHeight', Math.min(px, headMax))}
        onreset={() => sections.reset('messageHeight')}
      />
    {/if}
    <div class="part" class:closed={closed.files}>
    <h3>
      <button
        class="fold"
        aria-expanded={!closed.files}
        aria-controls="detail-files"
        onclick={() => sections.toggle('files')}
      >
        <span class="caret">
          <Icon name={closed.files ? 'chevronRight' : 'chevronDown'} size={13} />
        </span>
        {files.length} file{files.length === 1 ? '' : 's'}{compare
          ? files.length === 1
            ? ' differs'
            : ' differ'
          : ''}
      </button>
    </h3>
    {#if !closed.files}
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
    <div class="scroll" id="detail-files">
    {#if showAll}
      {#if loadingAll}
        <p class="muted">Reading the tree…</p>
      {:else if allError}
        <p class="error">{allError}</p>
      {:else}
        <p class="muted count">
          {everything.length} file{everything.length === 1 ? '' : 's'} at this commit
        </p>
        <FileTree nodes={tree} {openPath} {onOpenFile} {onFileMenu} {onFolderMenu} {locks} startClosed />
      {/if}
    {:else if grouping === 'tree'}
      <FileTree nodes={tree} {openPath} {onOpenFile} {onFileMenu} {onFolderMenu} {locks} />
    {:else}
    <ul class="files">
      {#each shown as file (file.path)}
        <li>
          <button
            class="file"
            class:open={file.path === openPath}
            onclick={() => onOpenFile(file.path)}
            oncontextmenu={(event) => onFileMenu?.(event, file.path)}
            title={file.oldPath ? `${file.path}\nfrom ${file.oldPath}` : file.path}
          >
            <span class="mark {file.change}">{mark[file.change] ?? '?'}</span>
            <span class="path">
              <span class="dir">{split(file.path).dir}</span
              ><span class="name">{split(file.path).name}</span>
            </span>
            <LockMark lock={locks.get(file.path)} />
          </button>
        </li>
      {/each}
      {#if files.length > LIMIT}
        <li class="muted">…and {files.length - LIMIT} more</li>
      {/if}
    </ul>
    {/if}
    </div>
    {/if}
    </div>
  {/if}
</aside>

<style>
  /*
   * Two parts that scroll on their own: what the commit says, and what it changed. As one
   * scrolling column, a kernel merge's message pushed the file list a screen down, and a
   * seventeen-thousand-file list scrolled the message away for good.
   */
  aside {
    width: var(--details-w, 360px); flex: 0 0 auto; min-height: 0; overflow: hidden;
    display: flex; flex-direction: column;
    border: var(--hairline) solid var(--border); border-radius: var(--radius-3); background: var(--bg-0);
    padding: var(--space-3); font-size: var(--text-base);
  }
  .head { flex: 1 1 auto; min-height: 0; overflow-y: auto; }
  /* Undragged, the message is as tall as it is, up to half the panel; the files have the rest.
     The room at its foot is outside the scrolling, so a long message that scrolls ends clear of
     the grip rather than running under it. */
  .head.shared { flex: 0 1 auto; max-height: 50%; margin-bottom: var(--space-3); }
  .head.shared.sized { flex: 0 0 auto; max-height: none; }
  .head.closed { flex: 0 0 auto; }
  .part { flex: 1 1 0; min-height: 0; display: flex; flex-direction: column; }
  .part.closed { flex: 0 0 auto; }
  .part > h3 { flex: 0 0 auto; margin-top: var(--space-3); }
  .filebar { flex: 0 0 auto; }
  .scroll { flex: 1 1 auto; min-height: 0; overflow-y: auto; }
  .fold {
    display: flex; align-items: flex-start; gap: var(--space-1); cursor: pointer; text-align: left;
    font: inherit; color: inherit; text-transform: inherit; letter-spacing: inherit;
    padding: 0; margin: 0; background: var(--bg-0); border: 0; border-radius: var(--radius-0);
  }
  h3 .fold { align-items: center; }
  .fold:hover { color: var(--fg-0); }
  .fold:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
  .caret { flex: 0 0 auto; display: flex; color: var(--fg-2); }
  h2 .caret { margin-top: 0.2em; }
  /*
   * Every element that carries text in this panel paints its own opaque background.
   *
   * The panel's own `background` is not enough. It scrolls, and WebKit paints a scrolling
   * container's background into one layer and its contents into another; the contents layer is
   * transparent, so text on it drops from subpixel to grayscale antialiasing and the whole
   * column reads soft. Naming the colour on the text elements themselves is what fixes it.
   */
  h2, h3, dt, dd, .muted, .error, .all, .name, .authored, .when { background: var(--bg-0); }
  h2 {
    font-size: var(--text-lg); font-weight: var(--weight-semibold); line-height: 1.3;
    margin: 0 0 var(--space-3); color: var(--fg-0);
    /* Balanced, so a two-line summary does not leave one word alone on the second line. */
    text-wrap: balance;
  }
  /*
   * Summary and body in one box on the panel's own ground, edged by a hairline. The box is the
   * frame, so the summary inside it is set a step down from a free-standing heading, and the
   * body needs no plate or rule of its own.
   */
  .message {
    margin-bottom: var(--space-3); padding: var(--space-2) var(--space-3);
    border: var(--hairline) solid var(--border); border-radius: var(--radius-1);
    background: var(--bg-0);
  }
  .message h2 { font-size: var(--text-md); margin-bottom: var(--space-2); }
  .message.bare h2 { margin-bottom: 0; }

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
  .face img { width: 100%; height: 100%; border-radius: 50%; object-fit: cover; }
  .face {
    flex: 0 0 auto; width: 22px; height: 22px; border-radius: 50%; overflow: hidden;
    display: flex; align-items: center; justify-content: center;
    font-size: var(--text-mark); font-weight: var(--weight-bold); letter-spacing: 0.02em;
    /* White in both themes, which is why the node palette is dark in both. */
    color: #ffffff;
  }
  .names { display: flex; flex-direction: column; min-width: 0; line-height: 1.25; }
  .name {
    font-size: var(--text-md); font-weight: var(--weight-semibold); color: var(--fg-0);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .authored {
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
    font-size: var(--text-xs); font-weight: var(--weight-semibold);
    text-transform: uppercase; letter-spacing: var(--tracking-label);
    color: var(--fg-2); margin: var(--space-4) 0 var(--space-2);
  }
  h3::after {
    content: ''; flex: 1; height: var(--hairline); background: var(--border);
  }
  /* Going back to one commit, which is a step out of a mode rather than an action on the
     repository, so it is set as a link rather than as a button with a fill. */
  .single {
    align-self: flex-start; font: inherit; font-size: var(--text-base); cursor: pointer;
    background: none; border: 0; padding: 0; color: var(--accent);
  }
  .single:hover { text-decoration: underline; }
  .ends { display: flex; gap: var(--space-4); }
  .end { color: var(--fg-0); font-weight: var(--weight-semibold); margin-right: var(--space-2); }
  .note { margin: var(--space-2) 0; }
  .since {
    display: flex; align-items: center; gap: var(--space-2); margin: var(--space-2) 0;
    color: var(--fg-1); background: var(--bg-0);
  }
  .ref {
    font: inherit; padding: 0; border: 0; cursor: pointer; background: none;
    color: var(--accent); text-decoration: underline; text-underline-offset: 2px;
  }
  .mentions { list-style: none; margin: 0 0 var(--space-4); padding: 0; }
  .mentions button {
    display: flex; align-items: baseline; gap: var(--space-2); width: 100%; text-align: left;
    font: inherit; font-size: var(--text-sm); cursor: pointer; padding: 2px var(--space-2);
    background: var(--bg-0); border: 0; border-radius: var(--radius-1); color: var(--fg-1);
  }
  .mentions button:hover { background: var(--bg-2); color: var(--fg-0); }
  .mentions .state {
    flex: 0 0 auto; font-size: var(--text-xs); padding: 0 4px; border-radius: var(--radius-0);
    border: 1px solid var(--border-strong); color: var(--fg-2);
  }
  .mentions .state.open { color: var(--accent); border-color: var(--accent-line); }
  .mentions .num { flex: 0 0 auto; color: var(--fg-2); font-family: var(--font-mono); }
  .mentions .title { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /*
   * In the interface face, as prose. Set in the code face it read as a terminal dump, and a
   * message is written to be read; `pre-wrap` keeps its line breaks and indentation, which is
   * what a pasted log or a list of points needs from it.
   */
  .body {
    margin: 0; font-family: var(--font-ui); font-size: var(--text-base);
    line-height: var(--leading-body);
    white-space: pre-wrap; word-break: break-word; color: var(--fg-1);
    background: var(--bg-0);
  }
  /*
   * At the panel's own split, a long message scrolls inside its box, as the reference does,
   * so who wrote it and the facts under it stay in view. Dragging the divider is asking for
   * more message, so a head that has been sized lets the box grow with it.
   */
  .head.shared:not(.sized) .message .body { max-height: 16em; overflow-y: auto; }
  /* A note stands outside the message box, so it keeps a plate to be told apart by. */
  .note .body {
    margin-bottom: var(--space-4); padding: var(--space-2) var(--space-3);
    background: var(--bg-1); border-radius: var(--radius-1);
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
  /* The body's own frame is the note's: one box, headed, with its edit link on the heading. */
  .note { margin: 0 0 var(--space-3); position: relative; }
  .note h3 { margin: 0 0 var(--space-1); }
  .edit-note {
    position: absolute; top: 0; right: 0; cursor: pointer;
    font: inherit; font-size: var(--text-sm); color: var(--accent); background: var(--bg-0); border: 0; padding: 0;
  }
  .picked { list-style: none; margin: 0 0 var(--space-3); padding: 0; display: grid; gap: var(--space-1); }
  .picked li { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--fg-1); }
  .picked .mono { color: var(--fg-2); margin-right: var(--space-1); }
  .signature.ok { color: var(--ok); }
  .signature.warn { color: var(--warn); }
  .signature.danger { color: var(--danger); }
  .why { display: block; color: var(--fg-2); font-size: var(--text-sm); }
  .copy {
    flex: 0 0 auto; display: flex; padding: 2px; margin-top: -1px; cursor: pointer;
    background: var(--bg-0); border: 0; border-radius: var(--radius-1); color: var(--fg-2);
    transition: color var(--fast) var(--ease), background var(--fast) var(--ease);
  }
  .copy:hover { color: var(--fg-0); background: var(--bg-1); }
  .parent { display: block; }
  .muted { color: var(--fg-2); }
  .error { color: var(--danger); }
  .toggle.hidden { visibility: hidden; }
  .count { padding-bottom: 0; }
  .filebar {
    display: flex; align-items: center; gap: var(--space-3);
    margin-bottom: var(--space-2);
  }
  /* A segmented control: the choice that is on is a raised face in a sunken track. */
  .toggle {
    display: flex; gap: var(--space-0); padding: var(--space-0);
    background: var(--bg-1); border-radius: var(--radius-1);
  }
  .toggle button {
    font: inherit; font-size: var(--text-sm); cursor: pointer; padding: 1px var(--space-2);
    background: var(--bg-1); border: 0; border-radius: var(--radius-0); color: var(--fg-2);
  }
  .toggle button:hover { color: var(--fg-0); }
  .toggle button.on {
    background: var(--bg-0); color: var(--fg-0); font-weight: var(--weight-semibold);
    box-shadow: 0 0 0 var(--hairline) var(--border);
  }
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
    background: var(--bg-0); border: 0; border-radius: var(--radius-1); cursor: pointer;
    color: var(--fg-1); overflow: hidden;
    min-height: calc(var(--row-h) - 4px);
  }
  .file:hover { background: var(--bg-1); }
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
    border-radius: var(--radius-0); background: var(--bg-2); color: var(--fg-2);
  }
  .mark.added { color: var(--ok); background: var(--ok-soft); }
  .mark.deleted { color: var(--danger); background: var(--danger-soft); }
  .mark.modified { color: var(--lane-1); }
  .mark.renamed, .mark.copied { color: var(--lane-5); }
</style>
