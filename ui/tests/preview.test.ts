import { describe, expect, it } from 'vitest';

import { preview } from '../src/ipc/preview';

/**
 * The fixture engine, held to being a faithful stand-in.
 *
 * It exists so the window can be looked at and driven without a repository, which only works
 * while it answers everything the window asks. It used to answer `null` for anything it had no
 * fixture for, and that is a worse failure than none: `repo_stashes` came back null, the state
 * assigned it to its list, the sidebar threw reading `.length`, and the effect that mounts the
 * panel died with it — so hiding the left panel in a browser was permanent, while the real
 * window was fine. A harness that lies is worse than no harness.
 */
describe('the preview engine', () => {
  it('refuses a command it has no answer for, rather than saying null', () => {
    expect(() => preview('a_command_nobody_wrote', {})).toThrow(/no answer for/);
  });

  it('still answers the commands whose real answer is null', () => {
    // `null` in the table is a fixture, not a hole, so the two have to be told apart by
    // whether the key is there rather than by what it holds.
    expect(preview('unwatch_repo', {})).toBeNull();
    expect(preview('graph_rewalk', {})).toBeNull();
  });

  it('answers every list the sidebar counts with an array', () => {
    // Each of these is assigned straight into a state and then read for its length.
    const lists = ['repo_refs', 'repo_stashes', 'repo_submodules', 'recent_repos',
                   'hosting_pull_requests', 'remote_list'];
    for (const command of lists) {
      expect(Array.isArray(preview(command, {})), command).toBe(true);
    }
  });

  it('answers the scope with both of its fields, since the sidebar reads them', () => {
    const scope = preview('graph_scope', {}) as { solo: unknown; hidden: unknown };
    expect(scope.solo).toBeNull();
    expect(Array.isArray(scope.hidden)).toBe(true);
  });
});
