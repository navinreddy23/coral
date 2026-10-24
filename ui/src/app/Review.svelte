<script lang="ts">
  import { shortAge } from './age';
  import type { PullRequest, Reply } from '../ipc/commands';
  import type { ReviewState } from '../state/review.svelte';

  const { review, pr, label, canRequestChanges, onClose }: {
    review: ReviewState;
    pr: PullRequest;
    /** What the host calls one: GitHub says pull, GitLab says merge. */
    label: string;
    /** GitHub can; GitLab has no verdict short of approving for a merge request. */
    canRequestChanges: boolean;
    onClose: () => void;
  } = $props();

  let body = $state('');

  const VERDICT = { approved: 'approved', changesRequested: 'asked for changes' } as const;

  function age(at: string): string {
    const when = Date.parse(at);
    return Number.isNaN(when) ? '' : shortAge(when / 1000);
  }

  async function send(kind: Reply['kind']) {
    if (await review.reply({ kind, body: body.trim() })) body = '';
  }

  function key(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      onClose();
      event.preventDefault();
    }
  }
</script>

<svelte:window onkeydown={key} />

<div class="panel">
  <header>
    <button class="back" onclick={onClose}>← Close</button>
    <p class="heading">#{pr.number} {pr.title}</p>
    <p class="sub">{label} by {pr.author}, {pr.sourceBranch} into {pr.targetBranch}</p>
  </header>

  <div class="body">
    {#if review.loading && review.remarks.length === 0}
      <p class="muted">Reading the review…</p>
    {:else if review.remarks.length === 0}
      <p class="muted">Nobody has said anything on it yet.</p>
    {:else}
      <ol class="remarks">
        {#each review.remarks as remark, i (i)}
          <li class:approved={remark.verdict === 'approved'} class:changes={remark.verdict === 'changesRequested'}>
            <p class="who">
              <span class="author">{remark.author}</span>
              {#if remark.verdict}<span class="verdict">{VERDICT[remark.verdict]}</span>{/if}
              {#if remark.path}
                <span class="where">{remark.path}{remark.line === null ? '' : `:${remark.line}`}</span>
              {/if}
              <span class="when">{age(remark.at)}</span>
            </p>
            {#if remark.body}<pre class="text">{remark.body}</pre>{/if}
          </li>
        {/each}
      </ol>
    {/if}

    <div class="answer">
      <textarea
        bind:value={body}
        rows="4"
        disabled={review.busy}
        placeholder="Say something on it. Markdown, as on the host."
      ></textarea>
      {#if review.error !== null}<p class="bad">{review.error}</p>{/if}
      <div class="acts">
        <button class="go" disabled={review.busy || body.trim() === ''} onclick={() => void send('comment')}>
          Comment
        </button>
        <button class="approve" disabled={review.busy} onclick={() => void send('approve')}>
          Approve
        </button>
        {#if canRequestChanges}
          <!-- GitHub refuses a request for changes that says nothing about what to change. -->
          <button
            class="changes"
            disabled={review.busy || body.trim() === ''}
            onclick={() => void send('requestChanges')}
          >
            Request changes
          </button>
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .panel {
    position: absolute; inset: 0; z-index: 12; display: flex; flex-direction: column;
    background: var(--bg-0);
  }
  header { padding: var(--space-2); border-bottom: 1px solid var(--border); background: var(--bg-1); }
  .back {
    font: inherit; font-size: var(--text-base); cursor: pointer; padding: var(--space-2);
    border-radius: var(--radius-1); background: var(--bg-1); border: 0; color: var(--accent);
  }
  .back:hover { background: var(--bg-2); }
  .heading {
    margin: var(--space-2) 0 0 var(--space-2); font-size: var(--text-lg); font-weight: var(--weight-semibold);
    color: var(--fg-0);
  }
  .sub { margin: var(--space-1) 0 0 var(--space-2); font-size: var(--text-base); color: var(--fg-2); }
  .body { padding: var(--space-4); overflow-y: auto; display: flex; flex-direction: column; gap: var(--space-4); max-width: 52em; }
  .remarks { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: var(--space-3); }
  .remarks li { border-left: 2px solid var(--border-strong); padding-left: var(--space-3); }
  .remarks li.approved { border-left-color: var(--ok); }
  .remarks li.changes { border-left-color: var(--warn); }
  .who { margin: 0; display: flex; gap: var(--space-2); align-items: baseline; font-size: var(--text-base); }
  .author { font-weight: var(--weight-semibold); color: var(--fg-0); }
  .verdict { color: var(--fg-1); }
  .approved .verdict { color: var(--ok); }
  .changes .verdict { color: var(--warn); }
  .where { font-family: var(--font-mono); font-variant-ligatures: none; font-size: var(--text-sm); color: var(--fg-2); }
  .when { margin-left: auto; color: var(--fg-2); font-size: var(--text-sm); }
  .text {
    margin: var(--space-1) 0 0; white-space: pre-wrap; word-break: break-word;
    font: inherit; font-size: var(--text-base); color: var(--fg-1);
  }
  .answer { display: flex; flex-direction: column; gap: var(--space-2); }
  textarea {
    font: inherit; font-size: var(--text-base); color: var(--fg-0); resize: vertical;
    padding: var(--space-2); border-radius: var(--radius-1);
    background: var(--bg-1); border: 1px solid var(--border-strong);
  }
  .acts { display: flex; gap: var(--space-2); }
  .acts button {
    font: inherit; font-size: var(--text-md); cursor: pointer;
    padding: var(--space-2) var(--space-4); border-radius: var(--radius-1);
    background: var(--bg-1); border: 1px solid var(--border-strong); color: var(--fg-0);
  }
  .acts .go { background: var(--accent); border-color: var(--accent); color: var(--accent-fg); }
  .acts .approve { color: var(--ok); }
  .acts .changes { color: var(--warn); }
  .acts button:disabled { background: var(--bg-2); border-color: var(--border-strong); color: var(--fg-2); cursor: default; }
  .bad { margin: 0; color: var(--danger); font-size: var(--text-base); }
  .muted { margin: 0; color: var(--fg-2); font-size: var(--text-base); }
</style>
