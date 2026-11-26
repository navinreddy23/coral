// @vitest-environment happy-dom
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { cleanup, render } from '@testing-library/svelte';
import { fireEvent, waitFor } from '@testing-library/dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

import Staging from '../../src/app/Staging.svelte';
import type { MenuItem } from '../../src/app/Menu.svelte';
import { CommitState } from '../../src/state/commit.svelte';
import { SectionsState } from '../../src/state/sections.svelte';
import { SpellingState } from '../../src/state/spelling.svelte';
import { ViewsState } from '../../src/state/views.svelte';
import { WorktreeState } from '../../src/state/worktree.svelte';
import type { Status } from '../../src/ipc/types';

/** Misspellings the stand-in engine knows. */
let typos: string[] = [];

function found(text: string) {
  return [...text.matchAll(/\p{L}+/gu)]
    .filter((m) => typos.includes(m[0].toLowerCase()))
    .map((m) => ({ word: m[0], start: m.index, end: m.index + m[0].length }));
}

function engine(command: string, args: Record<string, string>): unknown {
  switch (command) {
    case 'spelling_check':
      return { summary: found(args['summary'] ?? ''), body: found(args['body'] ?? '') };
    case 'spelling_suggest':
      return args['word'] === 'teh' ? ['the', 'tech'] : [];
    case 'spelling_learn':
      typos = typos.filter((t) => t !== args['word']);
      return null;
    default:
      return {
        entries: [],
        branch: 'main',
        upstream: null,
        ahead: 0,
        behind: 0,
      } as unknown as Status;
  }
}

/*
 * happy-dom lays nothing out, so every box is at the origin and nothing is under any point.
 * The layer's clip is given the field's box, and each underlined word a box of its own along
 * one line, ten pixels to a character, which is enough for a point to land on a word or miss.
 */
function layOut(): void {
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
    this: HTMLElement,
  ) {
    return new DOMRect(0, 0, 600, 30);
  });
  vi.spyOn(HTMLElement.prototype, 'getClientRects').mockImplementation(function (
    this: HTMLElement,
  ) {
    const start = Number(this.dataset['start'] ?? 0);
    const width = (this.textContent?.length ?? 0) * 10;
    const rects = [new DOMRect(start * 10, 5, width, 20)];
    return Object.assign(rects, { item: (i: number) => rects[i] ?? null }) as unknown as DOMRectList;
  });
}

async function panel(over: Record<string, unknown> = {}) {
  const worktree = new WorktreeState();
  await worktree.load('/repo');
  const commit = new CommitState();
  const views = new ViewsState();
  const spelling = new SpellingState(views);
  const onWordMenu = vi.fn<(at: { x: number; y: number }, items: MenuItem[], keyed: boolean) => void>();
  const rendered = render(Staging, {
    props: {
      worktree,
      commit,
      sections: new SectionsState(),
      branch: 'main',
      openPath: null,
      grouping: 'tree',
      onGrouping: () => {},
      onDiscard: () => {},
      onFileMenu: () => {},
      onCommit: () => {},
      onOpenFile: () => {},
      spelling,
      onWordMenu,
      ...over,
    },
  });
  const summary = rendered.container.querySelector('input.summary') as HTMLInputElement;
  const body = rendered.container.querySelector('textarea.description') as HTMLTextAreaElement;
  return { ...rendered, commit, views, spelling, onWordMenu, summary, body };
}

/** Types `value` into `field` and leaves it, so nothing is held back as still being typed. */
async function write(field: HTMLInputElement | HTMLTextAreaElement, value: string) {
  await fireEvent.input(field, { target: { value } });
  await fireEvent.blur(field);
}

function underlined(container: HTMLElement): string[] {
  return [...container.querySelectorAll('.misspellings .miss')].map((m) => m.textContent ?? '');
}

async function marked(container: HTMLElement, words: string[]) {
  await waitFor(() => expect(underlined(container)).toEqual(words));
}

function labels(items: MenuItem[]): string[] {
  return items.map((i) => (i.kind === 'separator' ? '—' : i.label));
}

function choose(items: MenuItem[], label: string): void {
  const item = items.find((i) => i.kind === 'item' && i.label === label);
  if (item?.kind !== 'item') throw new Error(`no ${label} in ${labels(items).join(', ')}`);
  item.run();
}

beforeEach(() => {
  localStorage.clear();
  typos = ['teh', 'recieve', 'colour'];
  invoke.mockReset();
  invoke.mockImplementation(async (command: string, args: Record<string, string>) =>
    engine(command, args),
  );
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('misspelt words in the commit box', () => {
  it('are underlined in both fields once typing pauses, over a copy of the text', async () => {
    const { container, summary, body } = await panel();
    await write(summary, 'Fix teh watcher');
    await write(body, 'It will recieve events.\nAll of them.');
    await marked(container, ['teh', 'recieve']);

    const layers = [...container.querySelectorAll('.misspellings')];
    expect(layers.map((l) => l.textContent)).toEqual([
      'Fix teh watcher',
      'It will recieve events.\nAll of them.',
    ]);
    // Over the field and out of the way: drawn on top, never in the way of a click or a
    // screen reader, which reads the field itself.
    for (const layer of layers) {
      expect(layer.getAttribute('aria-hidden')).toBe('true');
      expect(getComputedStyle(layer).pointerEvents).toBe('none');
    }
    expect(container.querySelector('.miss')?.getAttribute('data-start')).toBe('4');
  });

  it('are drawn above the field, which WebKitGTK raises while it has the caret', () => {
    const css = readFileSync(resolve(process.cwd(), 'src/app/Misspellings.svelte'), 'utf8');
    expect(css).toMatch(/\.misspellings \{[^}]*position: absolute; inset: 0; z-index: 1;/u);
  });

  it('leaves the webview to check nothing, so there is one set of underlines', async () => {
    const { summary, body } = await panel();
    expect(summary.getAttribute('spellcheck')).toBe('false');
    expect(body.getAttribute('spellcheck')).toBe('false');
  });

  it('are not checked at all without a spelling state', async () => {
    const { container, summary } = await panel({ spelling: null });
    await write(summary, 'Fix teh watcher');
    await new Promise((resolve) => setTimeout(resolve, 400));
    expect(container.querySelector('.misspellings')).toBeNull();
    expect(invoke.mock.calls.filter((c) => c[0] === 'spelling_check')).toEqual([]);
  });

  it('are not checked when Preferences says not to', async () => {
    const { container, summary, views } = await panel();
    views.set('spelling', 'off');
    await write(summary, 'Fix teh watcher');
    await new Promise((resolve) => setTimeout(resolve, 400));
    expect(container.querySelector('.misspellings')).toBeNull();
    expect(invoke.mock.calls.filter((c) => c[0] === 'spelling_check')).toEqual([]);
  });

  it('wait until the word being typed is finished', async () => {
    const { container, summary } = await panel();
    await fireEvent.input(summary, { target: { value: 'Fix teh' } });
    await new Promise((resolve) => setTimeout(resolve, 400));
    expect(underlined(container)).toEqual([]);
    await fireEvent.input(summary, { target: { value: 'Fix teh ' } });
    await marked(container, ['teh']);
  });

  it('follow a message filled in by something other than typing', async () => {
    const { container, commit } = await panel();
    commit.seed('Fix teh watcher', 'It recieves events');
    await marked(container, ['teh']);
  });

  it('are checked again in British English once that is chosen', async () => {
    const { container, summary, views } = await panel();
    await write(summary, 'Pick a colour');
    await marked(container, ['colour']);
    typos = ['color'];
    views.set('spelling', 'en-GB');
    await waitFor(() =>
      expect(invoke).toHaveBeenLastCalledWith('spelling_check', {
        summary: 'Pick a colour',
        body: '',
        dialect: 'en-GB',
      }),
    );
    await marked(container, []);
  });
});

describe('what a misspelt word offers', () => {
  it('opens on a right-click on the word, with suggestions and what else can be done', async () => {
    layOut();
    const { container, summary, onWordMenu } = await panel();
    await write(summary, 'Fix teh watcher');
    await marked(container, ['teh']);

    // "teh" starts at the fifth character: forty to seventy pixels along.
    const click = new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 55, clientY: 15 });
    summary.dispatchEvent(click);
    expect(click.defaultPrevented).toBe(true);
    await waitFor(() => expect(onWordMenu).toHaveBeenCalledTimes(1));
    const [at, items, keyed] = onWordMenu.mock.calls[0] ?? [];
    expect(at).toEqual({ x: 55, y: 15 });
    expect(keyed).toBe(false);
    expect(labels(items ?? [])).toEqual([
      'the',
      'tech',
      '—',
      'Add “teh” to dictionary',
      'Ignore “teh”',
    ]);
    expect(invoke).toHaveBeenCalledWith('spelling_suggest', { word: 'teh', dialect: 'en-US' });
  });

  it('leaves the usual menu alone anywhere but on a misspelt word', async () => {
    layOut();
    const { container, summary, onWordMenu } = await panel();
    await write(summary, 'Fix teh watcher');
    await marked(container, ['teh']);

    const click = new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 120, clientY: 15 });
    summary.dispatchEvent(click);
    expect(click.defaultPrevented).toBe(false);
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(onWordMenu).not.toHaveBeenCalled();
  });

  it('says when there is nothing to suggest', async () => {
    layOut();
    const { container, summary, onWordMenu } = await panel();
    await write(summary, 'A colour');
    await marked(container, ['colour']);
    summary.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 30, clientY: 15 }));
    await waitFor(() => expect(onWordMenu).toHaveBeenCalled());
    const items = onWordMenu.mock.calls[0]?.[1] ?? [];
    expect(items[0]).toMatchObject({ kind: 'item', label: 'No suggestions', disabled: true });
  });

  it('opens from the Menu key or Shift+F10 with the caret in the word', async () => {
    layOut();
    const { container, body, onWordMenu } = await panel();
    await write(body, 'Fix teh watcher');
    await marked(container, ['teh']);

    body.setSelectionRange(5, 5);
    const key = new KeyboardEvent('keydown', { key: 'F10', shiftKey: true, bubbles: true, cancelable: true });
    body.dispatchEvent(key);
    expect(key.defaultPrevented).toBe(true);
    await waitFor(() => expect(onWordMenu).toHaveBeenCalledTimes(1));
    // Under the word's first letter, where a menu from the keyboard belongs.
    expect(onWordMenu.mock.calls[0]?.[0]).toEqual({ x: 40, y: 25 });
    expect(onWordMenu.mock.calls[0]?.[2], 'and marked as opened from the keyboard').toBe(true);

    body.dispatchEvent(new KeyboardEvent('keydown', { key: 'ContextMenu', bubbles: true, cancelable: true }));
    await waitFor(() => expect(onWordMenu).toHaveBeenCalledTimes(2));

    // Not with the caret elsewhere: that is the usual menu's.
    body.setSelectionRange(12, 12);
    const elsewhere = new KeyboardEvent('keydown', { key: 'ContextMenu', bubbles: true, cancelable: true });
    body.dispatchEvent(elsewhere);
    expect(elsewhere.defaultPrevented).toBe(false);
  });

  it('puts a suggestion in place of the word, and only the word', async () => {
    layOut();
    const { container, summary, commit, onWordMenu } = await panel();
    await write(summary, 'Fix teh watcher, teh end');
    await marked(container, ['teh', 'teh']);

    summary.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 55, clientY: 15 }));
    await waitFor(() => expect(onWordMenu).toHaveBeenCalled());
    choose(onWordMenu.mock.calls[0]?.[1] ?? [], 'the');
    await waitFor(() => expect(commit.summary).toBe('Fix the watcher, teh end'));
    expect(summary.value).toBe('Fix the watcher, teh end');
    await marked(container, ['teh']);
  });

  it('adds the word to the dictionary, and takes its underline away', async () => {
    layOut();
    const { container, summary, onWordMenu } = await panel();
    await write(summary, 'Fix teh watcher');
    await marked(container, ['teh']);

    summary.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 55, clientY: 15 }));
    await waitFor(() => expect(onWordMenu).toHaveBeenCalled());
    choose(onWordMenu.mock.calls[0]?.[1] ?? [], 'Add “teh” to dictionary');
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('spelling_learn', { word: 'teh' }));
    await marked(container, []);
  });

  it('ignores the word for the rest of the session, everywhere it appears', async () => {
    layOut();
    const { container, summary, body, onWordMenu } = await panel();
    await write(summary, 'Fix teh watcher');
    await write(body, 'Teh body');
    await marked(container, ['teh', 'Teh']);

    summary.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 55, clientY: 15 }));
    await waitFor(() => expect(onWordMenu).toHaveBeenCalled());
    choose(onWordMenu.mock.calls[0]?.[1] ?? [], 'Ignore “teh”');
    await marked(container, []);
    expect(invoke.mock.calls.some((c) => c[0] === 'spelling_learn')).toBe(false);
  });
});
