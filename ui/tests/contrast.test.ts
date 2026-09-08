import { describe, expect, it } from 'vitest';

import { apart, contrast, token, type Theme } from './stylesheet';

/**
 * The accessibility pass, written down.
 *
 * Every ratio the token file claims in a comment is asserted here, in both themes. Comments
 * are how those numbers used to be recorded, and a comment does not notice when somebody
 * lightens a grey by two points.
 *
 * The thresholds: 7:1 is the enhanced bar for body text, which this window uses because it
 * sets almost everything between 10 and 13px. 4.5:1 is the ordinary bar, used for text that
 * is already large or already coloured to mean something. 3:1 is the bar for a control's
 * edge or a graphic, which is all a lane stroke or the brand mark ever is.
 */
const THEMES: Theme[] = ['light', 'dark'];

/** Reads a token and fails with its name rather than with `undefined`. */
function colour(theme: Theme, name: string): string {
  return token(theme, name);
}

describe.each(THEMES)('the %s palette', (theme) => {
  const on = (fg: string, bg: string) => contrast(colour(theme, fg), colour(theme, bg));

  it('sets every step of the text ladder above the enhanced threshold', () => {
    // Measured against the panel, not the page: most of the dim text in this window is on
    // `bg-1`, and a ratio computed against the page flatters itself by half a point.
    for (const ground of ['bg-0', 'bg-1']) {
      expect(on('fg-0', ground), `fg-0 on ${ground}`).toBeGreaterThanOrEqual(15);
      expect(on('fg-1', ground), `fg-1 on ${ground}`).toBeGreaterThanOrEqual(10);
      expect(on('fg-2', ground), `fg-2 on ${ground}`).toBeGreaterThanOrEqual(7);
    }
  });

  it('keeps the ladder in order, so a dimmer step is never the brighter one', () => {
    expect(on('fg-0', 'bg-0')).toBeGreaterThan(on('fg-1', 'bg-0'));
    expect(on('fg-1', 'bg-0')).toBeGreaterThan(on('fg-2', 'bg-0'));
  });

  it('makes the accent legible as text, not only as a fill', () => {
    // It labels a link and a count as often as it fills a button.
    expect(on('accent', 'bg-0')).toBeGreaterThanOrEqual(4.5);
    expect(on('accent', 'bg-1')).toBeGreaterThanOrEqual(4.5);
  });

  it('keeps what is written on a filled accent readable', () => {
    // There is no equivalent for the brand, and that is the point: nothing is ever written
    // on it, so it has no foreground token to check.
    expect(on('accent-fg', 'accent')).toBeGreaterThanOrEqual(4.5);
  });

  it('keeps text on a tinted selection readable', () => {
    // A selected row is tinted rather than filled precisely so the text on it survives.
    expect(on('fg-0', 'accent-soft')).toBeGreaterThanOrEqual(7);
    expect(on('fg-1', 'accent-soft')).toBeGreaterThanOrEqual(4.5);
  });

  it('makes every state colour legible as text', () => {
    for (const state of ['danger', 'ok', 'warn']) {
      expect(on(state, 'bg-0'), `${state} on bg-0`).toBeGreaterThanOrEqual(4.5);
      expect(on(state, 'bg-1'), `${state} on bg-1`).toBeGreaterThanOrEqual(4.5);
    }
  });

  it('keeps text on a state tint readable', () => {
    for (const state of ['danger', 'ok', 'warn', 'brand']) {
      expect(on('fg-0', `${state}-soft`), `fg-0 on ${state}-soft`).toBeGreaterThanOrEqual(7);
    }
  });

  it('leaves code on a diff tint as readable as code anywhere else', () => {
    for (const tint of ['add-bg', 'remove-bg', 'add-word', 'remove-word']) {
      expect(on('fg-0', tint), `fg-0 on ${tint}`).toBeGreaterThanOrEqual(7);
    }
  });

  it('draws every lane strongly enough to be followed across a screen', () => {
    // A lane is a 2px stroke, so it needs the graphic threshold and then some: at 3:1 the
    // thinner ones read as a smudge rather than as a line.
    for (let lane = 1; lane <= 8; lane++) {
      expect(on(`lane-${lane}`, 'bg-0'), `lane-${lane}`).toBeGreaterThanOrEqual(4.5);
    }
  });

  it('keeps a label on a lane tint readable', () => {
    // Branch pills and the strip beside a row are lane-tinted and carry 10px text.
    for (let lane = 1; lane <= 8; lane++) {
      expect(on('fg-0', `lane-${lane}-soft`), `fg-0 on lane-${lane}-soft`).toBeGreaterThanOrEqual(7);
    }
  });

  it('keeps the brand visible without pretending it is text', () => {
    // It is a mark, a tab edge, a ring and a marker, never a word. The graphic threshold is
    // the honest one to hold it to.
    expect(on('brand', 'bg-0')).toBeGreaterThanOrEqual(3);
    expect(on('brand', 'bg-1')).toBeGreaterThanOrEqual(3);
  });

  it('separates the brand from danger, which are neighbours on the wheel', () => {
    // Perceptual distance, not contrast: two warm reds of the same lightness measure 1.0:1
    // against each other and are still indistinguishable. The dark pair began at 0.05,
    // which is a shade rather than a difference, and coral was moved toward orange and red
    // toward rose until they were twice that far apart.
    expect(apart(colour(theme, 'brand'), colour(theme, 'danger'))).toBeGreaterThanOrEqual(0.1);
  });

  it('separates a control at rest from the panel it sits on', () => {
    expect(on('bg-2', 'bg-1')).toBeGreaterThanOrEqual(1.06);
    expect(on('bg-3', 'bg-2')).toBeGreaterThanOrEqual(1.06);
  });

  it('draws a border that can be seen against both the page and a panel', () => {
    expect(on('border', 'bg-0')).toBeGreaterThanOrEqual(1.1);
    expect(on('border-strong', 'bg-1')).toBeGreaterThanOrEqual(1.4);
  });

  it('shows the terminal colours against the page a terminal is drawn on', () => {
    // Every hue except black, which is the one colour a terminal expects to disappear into a
    // dark background — that is what programs use it for, and forcing it to stand out would
    // make Coral's terminal the only one that does.
    const names = ['red', 'green', 'yellow', 'blue', 'magenta', 'cyan', 'white'];
    for (const name of names) {
      expect(on(`term-${name}`, 'bg-0'), `term-${name}`).toBeGreaterThanOrEqual(3);
      expect(on(`term-bright-${name}`, 'bg-0'), `term-bright-${name}`).toBeGreaterThanOrEqual(3);
    }
  });
});

describe('the commit node colours', () => {
  it('carry white initials in both themes, which is why they are their own set', () => {
    // They are deliberately not redefined for dark. A node filled from the dark lane palette
    // would put white on `--lane-3`, which measures 1.5:1.
    for (let node = 1; node <= 8; node++) {
      for (const theme of THEMES) {
        expect(contrast('#ffffff', colour(theme, `node-${node}`)), `node-${node} in ${theme}`)
          .toBeGreaterThanOrEqual(4.5);
      }
    }
  });
});
