import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  clampSize,
  fitHeight,
  SECTION_LIMITS,
  SectionsState,
} from '../src/state/sections.svelte';

/** Storage the tests can inspect, so the layout can be checked without a browser. */
function stubStorage(): Map<string, string> {
  const store = new Map<string, string>();
  vi.stubGlobal('localStorage', {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => store.set(k, v),
  });
  return store;
}

describe('right panel sections', () => {
  let store = new Map<string, string>();
  beforeEach(() => {
    store = stubStorage();
  });

  it('starts with everything open and nothing dragged', () => {
    const sections = new SectionsState();
    expect(sections.sizes).toEqual({ unstagedShare: 0.5, composeHeight: null, messageHeight: null });
    expect(Object.values(sections.collapsed).some(Boolean)).toBe(false);
  });

  it('never lets a drag take a section past its floor or ceiling', () => {
    const sections = new SectionsState();
    sections.resize('unstagedShare', -3);
    expect(sections.sizes.unstagedShare).toBe(SECTION_LIMITS.unstagedShare.min);
    sections.resize('unstagedShare', 7);
    expect(sections.sizes.unstagedShare).toBe(SECTION_LIMITS.unstagedShare.max);
    sections.resize('composeHeight', 4);
    expect(sections.sizes.composeHeight).toBe(SECTION_LIMITS.composeHeight.min);
    sections.resize('messageHeight', 1e9);
    expect(sections.sizes.messageHeight).toBe(SECTION_LIMITS.messageHeight.max);
  });

  it('keeps a share as a fraction and a height in whole pixels', () => {
    expect(clampSize('unstagedShare', 0.337)).toBeCloseTo(0.337);
    expect(clampSize('composeHeight', 212.6)).toBe(213);
  });

  it('remembers sizes and folds across a restart', () => {
    const before = new SectionsState();
    before.resize('unstagedShare', 0.3);
    before.resize('messageHeight', 240);
    before.toggle('staged');
    before.toggle('files');

    const after = new SectionsState();
    expect(after.sizes.unstagedShare).toBeCloseTo(0.3);
    expect(after.sizes.messageHeight).toBe(240);
    expect(after.sizes.composeHeight).toBeNull();
    expect(after.collapsed.staged).toBe(true);
    expect(after.collapsed.files).toBe(true);
    expect(after.collapsed.unstaged).toBe(false);
  });

  it('resets one split and leaves the others', () => {
    const sections = new SectionsState();
    sections.resize('unstagedShare', 0.2);
    sections.resize('composeHeight', 300);
    sections.reset('composeHeight');
    expect(sections.sizes.composeHeight).toBeNull();
    expect(sections.sizes.unstagedShare).toBeCloseTo(0.2);
    expect(new SectionsState().sizes.composeHeight).toBeNull();
  });

  it('opens a folded section and leaves an open one alone', () => {
    const sections = new SectionsState();
    sections.toggle('compose');
    sections.open('compose');
    expect(sections.collapsed.compose).toBe(false);
    sections.open('compose');
    expect(sections.collapsed.compose).toBe(false);
  });

  it('clamps what an older release stored, and ignores what it does not know', () => {
    store.set(
      'coral.sections',
      JSON.stringify({
        unstagedShare: 0.99,
        composeHeight: 10,
        messageHeight: 'tall',
        collapsed: { staged: true, sidebar: true, unstaged: 'yes' },
      }),
    );
    const sections = new SectionsState();
    expect(sections.sizes.unstagedShare).toBe(SECTION_LIMITS.unstagedShare.max);
    expect(sections.sizes.composeHeight).toBe(SECTION_LIMITS.composeHeight.min);
    expect(sections.sizes.messageHeight).toBeNull();
    expect(sections.collapsed).toEqual({
      unstaged: false,
      staged: true,
      compose: false,
      message: false,
      files: false,
    });
  });

  it('opens the window on a corrupt entry or with storage switched off', () => {
    store.set('coral.sections', '{not json');
    expect(new SectionsState().sizes.unstagedShare).toBe(0.5);
    vi.stubGlobal('localStorage', {
      getItem: () => {
        throw new Error('denied');
      },
      setItem: () => {
        throw new Error('denied');
      },
    });
    const sections = new SectionsState();
    sections.toggle('message');
    expect(sections.collapsed.message).toBe(true);
  });
});

describe('fitHeight', () => {
  it('draws a dragged height as it is when the panel has the room', () => {
    expect(fitHeight(200, 800, 96, 60)).toBe(200);
  });

  it('gives way so the neighbours keep their floor in a short window', () => {
    expect(fitHeight(700, 500, 96, 60)).toBe(404);
  });

  it('never draws below the section floor, however short the window', () => {
    expect(fitHeight(700, 100, 96, 60)).toBe(60);
  });

  it('leaves the height alone before the panel has been measured', () => {
    expect(fitHeight(300, 0, 96, 60)).toBe(300);
  });
});
