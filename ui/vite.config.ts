import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  // Component tests mount the real component, so the browser build of Svelte has to win over
  // the server one; without this a mounted component renders nothing at all.
  resolve: { conditions: process.env.VITEST ? ['browser'] : [] },
  // Tauri points devUrl at this exact port; a silent bump would leave the window blank.
  server: { port: 5173, strictPort: true },
  clearScreen: false,
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: { target: 'es2022', sourcemap: true },
  // Component styles are injected into the test document, so a rule can be checked the way the
  // window applies it rather than by reading the source for a selector.
  test: { css: true },
});
