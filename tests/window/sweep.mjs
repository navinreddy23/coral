/**
 * Every surface of the window, photographed in both themes.
 *
 * Nothing here asserts what a screenshot should look like: there is no baseline to compare
 * against, and a pixel comparison of an interface under active design fails on every commit
 * for the wrong reason. What it asserts is that each surface can still be *reached* and still
 * renders something — a stop that throws, or that comes back with an empty page, fails the
 * sweep. The pictures are for a person to look at afterwards.
 *
 * That covers the half `ui/tests/` cannot: the whole window assembled, at a real size, with
 * the panels that only appear three clicks into a repository.
 *
 * What it does not cover is WebKitGTK, which is what actually ships. Subpixel antialiasing,
 * the GTK scrollbars, the undecorated window and the terminal all behave differently there,
 * and the only way to see them is `just app` under a nested X server.
 */
import { mkdirSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

import { launch, pause } from './driver.mjs';

const APP = process.env['CORAL_UI'] ?? 'http://localhost:5173/';
const GALLERY = `${APP}preview/`;
const OUT = process.argv[2] ?? 'shots';

/**
 * One picture: where it lives, and how to get there from a page that has just loaded.
 *
 * The gallery stops mount one panel with fixture state; the window stops drive the whole
 * application. Both are the real components.
 */
const STOPS = [
  { name: 'graph', at: APP, go: async () => {} },
  { name: 'details', at: APP, go: (b) => b.click('li.row', 3) },
  { name: 'staging', at: APP, go: (b) => b.press('button.row.wip') },
  {
    name: 'diff',
    at: APP,
    go: async (b) => {
      await b.click('li.row', 3);
      await b.click('aside ul.files button', 0);
    },
  },
  { name: 'find', at: APP, go: (b) => b.hotkey('f', { ctrl: true }) },
  { name: 'palette', at: APP, go: (b) => b.hotkey('p', { ctrl: true }) },
  { name: 'shortcuts', at: APP, go: (b) => b.hotkey('/', { ctrl: true }) },
  { name: 'commit-menu', at: APP, go: (b) => b.rightClick('li.row', 4) },
  { name: 'preferences', at: APP, go: (b) => b.press('.tools .chrome', 0) },
  {
    name: 'appearance',
    at: APP,
    go: async (b) => {
      await b.press('.tools .chrome', 0);
      await b.press('.prefs nav .pane', 0);
    },
  },
  {
    name: 'sidebar-full',
    at: APP,
    go: async (b) => {
      // Remotes and tags start closed, which is right for a repository with hundreds of each
      // and wrong for a picture meant to show the panel with something in it.
      await b.press('.head', 1);
      await b.press('.head', 3);
    },
  },
  {
    name: 'rail',
    at: APP,
    go: (b) => b.press('[aria-label="Left panel"]', 0),
  },
  { name: 'gallery', at: GALLERY, go: async () => {} },
];

const browser = await launch({ width: 1440, height: 900 });
const failures = [];
try {
  for (const theme of ['light', 'dark']) {
    for (const stop of STOPS) {
      // Set through the same storage key the window reads, then reloaded, because the theme is
      // applied once on construction.
      await browser.go(stop.at);
      // The view choices are remembered, so a stop that folds a panel away would fold it away
      // for every stop after it. Each starts from the state the window ships in.
      await browser.eval(
        `localStorage.setItem('coral.theme', ${JSON.stringify(theme)});` +
        `localStorage.removeItem('coral.views'); true`,
      );
      await browser.go(stop.at === GALLERY ? `${GALLERY}?theme=${theme}` : stop.at);

      try {
        await stop.go(browser);
      } catch (error) {
        failures.push(`${stop.name} (${theme}): ${error.message}`);
      }
      await pause(400);

      const painted = await browser.eval('document.body.innerText.trim().length');
      if (painted < 20) failures.push(`${stop.name} (${theme}): the page came back blank`);

      await browser.shot(join(OUT, `${stop.name}-${theme}.png`));
    }
  }
} finally {
  await browser.close();
}

mkdirSync(OUT, { recursive: true });
const taken = readdirSync(OUT).filter((f) => f.endsWith('.png'));
const empty = taken.filter((f) => statSync(join(OUT, f)).size < 2000);
for (const file of empty) failures.push(`${file}: the picture is empty`);

console.log(`${taken.length} pictures in ${OUT}`);
if (failures.length > 0) {
  console.error(`\n${failures.length} stops failed:`);
  for (const failure of failures) console.error(`  ${failure}`);
  process.exit(1);
}
console.log('every surface reached and rendered');
