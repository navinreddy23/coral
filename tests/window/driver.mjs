/**
 * A browser, driven, with nothing installed to do it.
 *
 * Node has a global `WebSocket`, so speaking the Chrome DevTools Protocol needs no package:
 * Chrome is launched with a debugging port, its page target is found over the HTTP endpoint it
 * publishes, and everything after that is JSON over one socket. That matters here because
 * `just check` has to keep working on a machine with no browser, so nothing this file needs
 * may end up in `ui/package.json`.
 *
 * What it drives is the real application running on the fixture engine in
 * `ui/src/ipc/preview.ts` — the same Svelte components the window mounts, answering the same
 * `invoke` calls, with no repository and no Rust behind them.
 */
import { spawn } from 'node:child_process';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';

const CHROME = process.env['CORAL_CHROME'] ?? '/usr/bin/google-chrome';

export const pause = (ms) => new Promise((ok) => setTimeout(ok, ms));

/** Starts a headless Chrome and returns the handle everything else is done through. */
export async function launch({ width = 1440, height = 900, port = 9222 } = {}) {
  const profile = mkdtempSync(join(tmpdir(), 'coral-window-'));
  const child = spawn(CHROME, [
    '--headless=new',
    `--remote-debugging-port=${port}`,
    `--user-data-dir=${profile}`,
    `--window-size=${width},${height}`,
    '--hide-scrollbars',
    '--no-first-run',
    '--no-default-browser-check',
    '--disable-gpu',
    // A picture that changes with the machine's display scaling is a picture nobody can
    // compare against yesterday's.
    '--force-device-scale-factor=1',
    'about:blank',
  ], { stdio: 'ignore' });

  const target = await until(async () => {
    const res = await fetch(`http://127.0.0.1:${port}/json/list`);
    return (await res.json()).find((t) => t.type === 'page') ?? null;
  }, 'chrome never answered on its debugging port');

  const socket = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((ok, bad) => {
    socket.onopen = ok;
    socket.onerror = () => bad(new Error('could not open the devtools socket'));
  });

  let next = 1;
  const waiting = new Map();
  socket.onmessage = (event) => {
    const message = JSON.parse(event.data);
    const seat = waiting.get(message.id);
    if (!seat) return;
    waiting.delete(message.id);
    if (message.error) seat.bad(new Error(message.error.message));
    else seat.ok(message.result);
  };

  const send = (method, params = {}) => new Promise((ok, bad) => {
    const id = next++;
    waiting.set(id, { ok, bad });
    socket.send(JSON.stringify({ id, method, params }));
  });

  await send('Page.enable');
  await send('Runtime.enable');
  await send('Emulation.setDeviceMetricsOverride', {
    width, height, deviceScaleFactor: 1, mobile: false,
  });

  return {
    send,

    /** Loads a page and waits for the application to replace the boot screen. */
    async go(url) {
      await send('Page.navigate', { url });
      await pause(400);
      for (let i = 0; i < 40; i++) {
        const ready = await this.eval(
          'document.readyState === "complete" && !document.getElementById("boot")',
        );
        if (ready === true) break;
        await pause(150);
      }
      // The first frame is a commit-time walk that the topological one replaces. Photographing
      // between the two catches a graph in a shape it holds for a moment and never again.
      await pause(600);
    },

    async eval(expression) {
      const out = await send('Runtime.evaluate', {
        expression, awaitPromise: true, returnByValue: true,
      });
      if (out.exceptionDetails) throw new Error(`${out.exceptionDetails.text}: ${expression}`);
      return out.result.value;
    },

    /** What the desktop is asking for, for the theme that follows it. */
    async prefers(scheme) {
      await send('Emulation.setEmulatedMedia', {
        features: [{ name: 'prefers-color-scheme', value: scheme }],
      });
    },

    async shot(path) {
      const out = await send('Page.captureScreenshot', { format: 'png' });
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, Buffer.from(out.data, 'base64'));
    },

    async click(selector, index = 0) {
      const at = await this.box(selector, index);
      for (const type of ['mousePressed', 'mouseReleased']) {
        await send('Input.dispatchMouseEvent', {
          type, x: at.x, y: at.y, button: 'left', clickCount: 1,
        });
      }
      await pause(150);
    },

    async rightClick(selector, index = 0) {
      const at = await this.box(selector, index);
      for (const type of ['mousePressed', 'mouseReleased']) {
        await send('Input.dispatchMouseEvent', {
          type, x: at.x, y: at.y, button: 'right', clickCount: 1,
        });
      }
      await pause(200);
    },

    /**
     * Calls an element's own handler, without a pointer.
     *
     * For a control something else overlaps — the working-copy row sits under the first
     * commit's transparent hit target — where a real press lands on the wrong thing but a
     * person clicking the visible row would not.
     */
    async press(selector, index = 0) {
      const found = await this.eval(`(() => {
        const el = document.querySelectorAll(${JSON.stringify(selector)})[${index}];
        if (!el) return false;
        el.click();
        return true;
      })()`);
      if (!found) throw new Error(`nothing at ${selector}[${index}]`);
      await pause(250);
    },

    /**
     * A keyboard shortcut, dispatched on the window.
     *
     * `Input.dispatchKeyEvent` with a modifier held does not reach a `svelte:window onkeydown`
     * handler reliably in headless Chrome, and what is being photographed is the panel the
     * shortcut opens rather than the browser's key handling.
     */
    async hotkey(key, { ctrl = false, shift = false } = {}) {
      await this.eval(`(() => {
        window.dispatchEvent(new KeyboardEvent('keydown', {
          key: ${JSON.stringify(key)}, ctrlKey: ${ctrl}, shiftKey: ${shift},
          bubbles: true, cancelable: true,
        }));
        return true;
      })()`);
      await pause(300);
    },

    async box(selector, index = 0) {
      const at = await this.eval(`(() => {
        const el = document.querySelectorAll(${JSON.stringify(selector)})[${index}];
        if (!el) return null;
        el.scrollIntoView({ block: 'center' });
        const r = el.getBoundingClientRect();
        return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
      })()`);
      if (!at) throw new Error(`nothing at ${selector}[${index}]`);
      return at;
    },

    async close() {
      socket.close();
      child.kill('SIGTERM');
    },
  };
}

async function until(fn, complaint, tries = 60) {
  for (let i = 0; i < tries; i++) {
    try {
      const value = await fn();
      if (value) return value;
    } catch {
      // Not up yet.
    }
    await pause(200);
  }
  throw new Error(complaint);
}
