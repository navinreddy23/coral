// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../src/ipc/invoke', () => ({ invoke, isPreview: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));

import { TerminalState } from '../src/state/terminal.svelte';
import { ViewsState } from '../src/state/views.svelte';

function sent(command: string): Record<string, unknown>[] {
  return invoke.mock.calls
    .filter((c) => c[0] === command)
    .map((c) => c[1] as Record<string, unknown>);
}

describe('the terminal', () => {
  beforeEach(() => {
    localStorage.clear();
    invoke.mockReset();
    let next = 0;
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'terminal_open') {
        next += 1;
        return { id: next, shell: '/bin/zsh' };
      }
      return undefined;
    });
  });

  it('asks for whatever the machine would use when nothing is chosen', async () => {
    const views = new ViewsState();
    await new TerminalState(views).start('/repo', 80, 24);
    expect(sent('terminal_open')[0]).toMatchObject({ shell: null, login: null });
  });

  it('sends the chosen shell, trimmed', async () => {
    const views = new ViewsState();
    views.set('terminalShell', '  /usr/bin/fish  ');
    views.set('terminalLogin', true);
    await new TerminalState(views).start('/repo', 80, 24);
    expect(sent('terminal_open')[0]).toMatchObject({ shell: '/usr/bin/fish', login: true });
  });

  it('treats a shell field cleared back to spaces as no choice at all', async () => {
    // Otherwise the engine is asked to run a program with no name.
    const views = new ViewsState();
    views.set('terminalShell', '   ');
    await new TerminalState(views).start('/repo', 80, 24);
    expect(sent('terminal_open')[0]).toMatchObject({ shell: null });
  });

  it('keeps false apart from unset, since false is a choice', async () => {
    const views = new ViewsState();
    views.set('terminalLogin', false);
    await new TerminalState(views).start('/repo', 80, 24);
    expect(sent('terminal_open')[0]).toMatchObject({ login: false });
  });

  it('hands back the shell already running in a repository', async () => {
    // Showing the pane again must not start a second shell, or the first is left holding a
    // pseudo-terminal nothing can reach.
    const term = new TerminalState(new ViewsState());
    const first = await term.start('/repo', 80, 24);
    const second = await term.start('/repo', 80, 24);
    expect(second).toEqual(first);
    expect(sent('terminal_open')).toHaveLength(1);
  });

  it('gives each repository its own shell', async () => {
    const term = new TerminalState(new ViewsState());
    await term.start('/alpha', 80, 24);
    await term.start('/beta', 80, 24);
    expect(sent('terminal_open').map((c) => c.path)).toEqual(['/alpha', '/beta']);
  });

  it('starts the next one with the settings as they are now', async () => {
    // What the restart button in the pane relies on. Hiding the pane leaves the shell running
    // on purpose, so a changed shell only arrives once the running one has been ended.
    const views = new ViewsState();
    const term = new TerminalState(views);
    await term.start('/repo', 80, 24);

    views.set('terminalShell', '/usr/bin/fish');
    await term.start('/repo', 80, 24);
    expect(sent('terminal_open'), 'the running shell is reused, not restarted').toHaveLength(1);

    await term.stop('/repo');
    await term.start('/repo', 80, 24);
    const opens = sent('terminal_open');
    expect(opens).toHaveLength(2);
    expect(opens[1]).toMatchObject({ shell: '/usr/bin/fish' });
  });

  it('closes the shell it ends, and forgets it', async () => {
    const term = new TerminalState(new ViewsState());
    const opened = await term.start('/repo', 80, 24);
    await term.stop('/repo');
    expect(sent('terminal_close')).toEqual([{ id: opened?.id }]);
    // Ending it twice is not an error: the shell may have exited on its own first.
    await term.stop('/repo');
    expect(sent('terminal_close')).toHaveLength(1);
  });

  it('ends the shells of tabs that have closed, and only those', async () => {
    const term = new TerminalState(new ViewsState());
    const alpha = await term.start('/alpha', 80, 24);
    await term.start('/beta', 80, 24);
    await term.keepOnly(new Set(['/beta']));
    expect(sent('terminal_close')).toEqual([{ id: alpha?.id }]);
  });

  it('reports a shell that could not be started rather than throwing', async () => {
    invoke.mockImplementation(async () => {
      throw new Error('could not start /usr/bin/fish: No such file or directory');
    });
    const term = new TerminalState(new ViewsState());
    expect(await term.start('/repo', 80, 24)).toBeNull();
    expect(term.error).toContain('No such file or directory');
  });
});
