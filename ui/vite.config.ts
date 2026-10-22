import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  /*
   * Component tests need Svelte's browser build to win over its server one, or a mounted
   * component renders nothing. The key is added only under Vitest and never otherwise:
   * `conditions` replaces Vite's defaults rather than adding to them, so setting it to an
   * empty array for the real build resolves the server entry and `mount` disappears — the
   * window then opens on a blank page with `mount(...) is not available on the server`.
   */
  ...(process.env.VITEST ? { resolve: { conditions: ['browser'] } } : {}),
  // Tauri points devUrl at this exact port; a silent bump would leave the window blank.
  server: { port: 5173, strictPort: true },
  clearScreen: false,
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: { target: 'es2022', sourcemap: true },
  // Component styles are injected into the test document, so a rule can be checked the way the
  // window applies it rather than by reading the source for a selector.
  //
  // The first test in a suite that mounts the whole window pays for its first render, and on
  // GitHub's Windows runners that was close enough to the five-second default to fail one run
  // in a few. A test that hangs still fails, only later.
  test: { css: true, testTimeout: 20_000 },
});
