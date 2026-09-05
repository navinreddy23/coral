import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../src/ipc/invoke', () => ({ invoke }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

const { ScopeState } = await import('../src/state/scope.svelte');

/** Answers `set_graph_scope` with whatever it was sent, which is what Rust does when nothing
 *  needs pruning. */
function echoing() {
  invoke.mockReset();
  invoke.mockImplementation((cmd: string, args: { scope?: unknown }) =>
    cmd === 'set_graph_scope' ? Promise.resolve(args.scope) : Promise.resolve({ solo: null, hidden: [] }),
  );
}

describe('what the graph is walked from', () => {
  beforeEach(echoing);

  it('moves solo to the next branch rather than adding it', async () => {
    // One at a time, so the banner always has one name to give and one thing to undo.
    const scope = new ScopeState();
    await scope.setSolo('/repo', 'refs/heads/a');
    expect(scope.solo).toBe('refs/heads/a');

    await scope.setSolo('/repo', 'refs/heads/b');
    expect(scope.solo).toBe('refs/heads/b');
  });

  it('leaves solo when the branch already soloed is soloed again', async () => {
    const scope = new ScopeState();
    await scope.setSolo('/repo', 'refs/heads/a');
    await scope.setSolo('/repo', 'refs/heads/a');
    expect(scope.solo).toBe(null);
    expect(scope.showingAll).toBe(true);
  });

  it('keeps what is hidden while soloing, and gives it back after', async () => {
    // Otherwise soloing a branch quietly unhides the spike somebody hid last week, and they
    // find it back in the graph with no idea what put it there.
    const scope = new ScopeState();
    await scope.toggleHidden('/repo', 'refs/heads/spike');
    await scope.setSolo('/repo', 'refs/heads/a');

    expect(scope.hidden).toEqual(['refs/heads/spike']);

    await scope.setSolo('/repo', null);
    expect(scope.solo).toBe(null);
    expect(scope.hidden).toEqual(['refs/heads/spike']);
  });

  it('hides and unhides the same branch', async () => {
    const scope = new ScopeState();
    await scope.toggleHidden('/repo', 'refs/heads/spike');
    expect(scope.hidden).toEqual(['refs/heads/spike']);
    expect(scope.walks('refs/heads/spike')).toBe(false);
    expect(scope.walks('refs/heads/main')).toBe(true);

    await scope.toggleHidden('/repo', 'refs/heads/spike');
    expect(scope.hidden).toEqual([]);
    expect(scope.walks('refs/heads/spike')).toBe(true);
  });

  it('says only the soloed branch is walked, whatever is hidden', async () => {
    const scope = new ScopeState();
    await scope.toggleHidden('/repo', 'refs/heads/spike');
    await scope.setSolo('/repo', 'refs/heads/a');

    expect(scope.walks('refs/heads/a')).toBe(true);
    expect(scope.walks('refs/heads/main')).toBe(false);
    expect(scope.showingAll).toBe(false);
  });

  it('takes what Rust kept, not what it was asked to keep', async () => {
    // Rust drops a name the repository no longer has. Believing the request instead would put
    // a branch that was deleted this morning in the banner, with nothing to click on.
    invoke.mockReset();
    invoke.mockResolvedValue({ solo: null, hidden: [] });

    const scope = new ScopeState();
    await scope.setSolo('/repo', 'refs/heads/gone');
    expect(scope.solo).toBe(null);
  });

  it('opens a repository showing everything when its scope cannot be read', async () => {
    invoke.mockReset();
    invoke.mockRejectedValue(new Error('no'));

    const scope = new ScopeState();
    await scope.load('/repo');
    expect(scope.showingAll).toBe(true);
    expect(scope.error).not.toBe(null);
  });

  it('does the least that puts one branch back on screen', async () => {
    // Clicking a dimmed row asks for that row. Handing back every branch somebody hid on
    // purpose is a bigger answer than the question, and nothing on screen would say what did it.
    const scope = new ScopeState();
    await scope.toggleHidden('/repo', 'refs/heads/spike');
    await scope.toggleHidden('/repo', 'refs/heads/old');
    await scope.setSolo('/repo', 'refs/heads/a');

    await scope.reveal('/repo', 'refs/heads/spike');

    expect(scope.solo).toBe(null);
    expect(scope.hidden).toEqual(['refs/heads/old']);
    expect(scope.walks('refs/heads/spike')).toBe(true);
  });

  it('leaves solo when the soloed branch is hidden', async () => {
    // The two say opposite things about one ref, and the window cannot draw both: the banner
    // named it as the only branch shown while the panel struck its eye through, and its own
    // label went missing from a graph made of its commits.
    const scope = new ScopeState();
    await scope.setSolo('/repo', 'refs/heads/spike');
    await scope.toggleHidden('/repo', 'refs/heads/spike');

    expect(scope.solo).toBe(null);
    expect(scope.hidden).toEqual(['refs/heads/spike']);
    expect(scope.hides('refs/heads/spike')).toBe(true);
  });

  it('keeps the soloed branch labelled even if it is also on the hidden list', async () => {
    // Reachable by hand-editing scope.json. Solo wins in the engine, where `Tips::Only` never
    // consults the hidden list, so the label has to follow.
    const scope = new ScopeState();
    await scope.toggleHidden('/repo', 'refs/heads/spike');
    await scope.setSolo('/repo', 'refs/heads/spike');

    expect(scope.hides('refs/heads/spike')).toBe(false);
    expect(scope.walks('refs/heads/spike')).toBe(true);
  });

  it('forgets the repository being left', async () => {
    const scope = new ScopeState();
    await scope.setSolo('/repo', 'refs/heads/a');
    scope.clear();
    expect(scope.showingAll).toBe(true);
  });
});
