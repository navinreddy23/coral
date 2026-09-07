/**
 * The eight lane colours, as a CSS value.
 *
 * The graph names them, and tab groups and profiles borrow them so that nothing in the window
 * has to maintain a palette of its own. `soft` asks for the tinted form, which is what fills a
 * band; full strength is for a chip, a dot or an edge, where the colour has to be
 * unmistakable.
 */
export function laneColour(colour: string | undefined, soft = false): string {
  if (!colour) return 'transparent';
  return `var(--${colour.replace('lane', 'lane-')}${soft ? '-soft' : ''})`;
}
