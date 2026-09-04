// @vitest-environment happy-dom
import { render } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('../../src/ipc/invoke', () => ({ invoke: vi.fn(), isPreview: () => false }));
// The window subscribes to terminal output and to repository changes. Neither channel
// exists without the Tauri shell, and the real `listen` throws rather than returning.
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

import StatusBar from '../../src/app/StatusBar.svelte';
import type { PlacedRef } from '../../src/ipc/commands';

function head(ahead: number, behind: number): PlacedRef {
  return {
    name: 'refs/heads/master',
    short: 'master',
    kind: { kind: 'local_branch' },
    target: 'a'.repeat(40),
    peeled: null,
    upstream: 'origin/master',
    ahead,
    behind,
    row: 0,
  };
}

const base = {
  branch: 'master',
  head: undefined,
  commits: 1_481_528,
  changed: 0,
  gitVersion: '2.43.0',
  host: null,
  report: null,
  busy: false,
  onDismiss: () => {},
};

function bar(over: Record<string, unknown> = {}) {
  return render(StatusBar, { props: { ...base, ...over } });
}

describe('the status bar', () => {
  it('names the branch and the commit count', () => {
    const { container } = bar();
    expect(container.textContent).toContain('master');
    expect(container.textContent).toContain('1,481,528 commits');
    expect(container.textContent).toContain('git 2.43.0');
  });

  it('says detached rather than showing nothing', () => {
    expect(bar({ branch: null }).container.textContent).toContain('detached');
  });

  it('shows ahead and behind only when there is something to show', () => {
    expect(bar().container.querySelector('.tally')).toBeNull();
    const { container } = bar({ head: head(3, 1) });
    expect(container.querySelector('.ahead')?.textContent).toBe('↑3');
    expect(container.querySelector('.behind')?.textContent).toBe('↓1');
  });

  it('caps a count that has stopped being useful', () => {
    const { container } = bar({ head: head(4000, 0) });
    expect(container.querySelector('.ahead')?.textContent).toBe('↑99+');
  });

  it('reports what an action did, and dismisses on click', async () => {
    let dismissed = false;
    const { container } = bar({
      report: { text: 'merge side stopped on conflicts', tone: 'warn' },
      onDismiss: () => (dismissed = true),
    });
    const report = container.querySelector('.report') as HTMLButtonElement;
    expect(report.className).toContain('warn');
    await fireEvent.click(report);
    expect(dismissed).toBe(true);
  });

  it('names the host and says when there is no token for it', () => {
    const host = {
      host: { kind: 'gitlab' as const, origin: 'https://gitlab.com', owner: 'o', repo: 'r' },
      detail: null,
      signedIn: false,
    };
    const { container } = bar({ host });
    expect(container.querySelector('.host')?.textContent).toContain('GitLab');
    expect(container.querySelector('.host')?.textContent).toContain('no token');
  });

  it('marks itself while something is running', () => {
    expect(bar({ busy: true }).container.querySelector('.status')?.className).toContain('busy');
    // The report wins over the spinner: what happened beats what is happening.
    const { container } = bar({ busy: true, report: { text: 'pull', tone: 'ok' } });
    expect(container.querySelector('.working')).toBeNull();
  });
});
