// @vitest-environment happy-dom
import { render, waitFor } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../../src/ipc/invoke', () => ({ invoke }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

import CommitSigning from '../../src/app/CommitSigning.svelte';
import { SigningState } from '../../src/state/signing.svelte';
import type { SigningKey, SigningScopes } from '../../src/ipc/types';

const WORK = '0633C12121B1A10FADE103E39E9BC1B3B7C4AA29';
const PERSONAL = 'AAAA111122223333444455556666777788889999';
const OLD = 'BBBB111122223333444455556666777788889999';

const KEYS: SigningKey[] = [
  { id: WORK, label: 'Navin <navin@work.example>', expires: 4_000_000_000, expired: false },
  { id: PERSONAL, label: 'Navin <navin@home.example>', expires: null, expired: false },
  { id: OLD, label: 'Old Laptop <old@home.example>', expires: 1_600_000_000, expired: true },
];

/** The app level sets a personal key and no signing; nothing overridden yet. */
function scopes(over: Partial<SigningScopes> = {}): SigningScopes {
  return {
    effective: {
      format: 'openpgp',
      program: '',
      key: PERSONAL,
      signCommits: false,
      signTags: false,
    },
    global: {
      format: 'openpgp',
      program: '',
      key: PERSONAL,
      signCommits: false,
      signTags: false,
    },
    local: { format: null, program: null, key: null, signCommits: null, signTags: null },
    ...over,
  };
}

function pane(over: Partial<SigningScopes> = {}, keys = KEYS) {
  const signing = new SigningState();
  signing.scopes = scopes(over);
  signing.keys = keys;
  return { signing, ...render(CommitSigning, { props: { signing } }) };
}

function field<T extends HTMLElement>(container: HTMLElement, selector: string): T {
  const found = container.querySelector<T>(selector);
  expect(found, selector).not.toBeNull();
  return found as T;
}

describe('the commit signing pane', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async () => scopes());
  });

  it('starts on this repository, because a key belongs to one', async () => {
    const { container } = pane();
    const on = [...container.querySelectorAll('.level button')].find((b) =>
      b.className.includes('on'),
    );
    expect(on?.textContent?.trim()).toBe('This repository');
  });

  it('says when everything is inherited', async () => {
    const { container } = pane();
    expect(container.querySelector('.note')?.textContent).toContain('Inheriting everything');
  });

  it('says when the repository overrides, and offers to stop', async () => {
    const { signing, container } = pane({
      local: { format: null, program: null, key: WORK, signCommits: true, signTags: null },
      effective: {
        format: 'openpgp',
        program: '',
        key: WORK,
        signCommits: true,
        signTags: false,
      },
    });
    expect(container.querySelector('.note')?.textContent).toContain('Overriding');
    expect(signing.inheriting).toBe(false);

    await fireEvent.click(field<HTMLButtonElement>(container, '.note button'));
    await waitFor(() => {
      const call = invoke.mock.calls.find(([cmd]) => cmd === 'signing_set_repo');
      if (!call) throw new Error('nothing cleared');
      // Every field null: back to inheriting, not pinned to what it happened to inherit.
      expect(call[1]).toEqual({
        path: '',
        overrides: {
          format: null,
          program: null,
          key: null,
          signCommits: null,
          signTags: null,
        },
      });
    });
  });

  it('shows the repository form seeded from what it would inherit', async () => {
    // So turning one thing on does not silently blank everything else.
    const { container } = pane();
    await waitFor(() => {
      expect(field<HTMLSelectElement>(container, '#key').value).toBe(PERSONAL);
    });
  });

  it('switching to the app level shows the app values', async () => {
    const { container } = pane({
      local: { format: null, program: null, key: WORK, signCommits: null, signTags: null },
      effective: {
        format: 'openpgp',
        program: '',
        key: WORK,
        signCommits: false,
        signTags: false,
      },
    });
    await waitFor(() => {
      expect(field<HTMLSelectElement>(container, '#key').value).toBe(WORK);
    });

    const app = [...container.querySelectorAll('.level button')].find((b) =>
      b.textContent?.includes('All repositories'),
    );
    await fireEvent.click(app as HTMLButtonElement);
    await waitFor(() => {
      expect(field<HTMLSelectElement>(container, '#key').value).toBe(PERSONAL);
    });
  });

  it('saves the repository form as overrides, with an empty program cleared', async () => {
    // Seeded through the scopes rather than by driving the select: happy-dom resets a
    // `<select>` when the change event fires, so the form is set up the way the engine would
    // hand it over and the mapping on the way out is what is checked.
    const { container } = pane({
      local: { format: null, program: null, key: WORK, signCommits: true, signTags: null },
      effective: {
        format: 'openpgp',
        program: '',
        key: WORK,
        signCommits: true,
        signTags: false,
      },
    });
    await waitFor(() => field<HTMLSelectElement>(container, '#key'));
    await fireEvent.click(
      [...container.querySelectorAll('footer button')].at(-1) as HTMLButtonElement,
    );

    await waitFor(() => {
      const call = invoke.mock.calls.find(([cmd]) => cmd === 'signing_set_repo');
      if (!call) throw new Error('not saved');
      const overrides = (call[1] as { overrides: Record<string, unknown> }).overrides;
      expect(overrides['key']).toBe(WORK);
      expect(overrides['signCommits']).toBe(true);
      // An empty program means "git's default", which is an absence, not an empty string git
      // would try to run.
      expect(overrides['program']).toBeNull();
    });
  });

  it('turns a typed program into an override rather than clearing it', async () => {
    const { container } = pane();
    const program = field<HTMLInputElement>(container, '#prog');
    await fireEvent.input(program, { target: { value: '/usr/local/bin/gpg2' } });
    await fireEvent.click(
      [...container.querySelectorAll('footer button')].at(-1) as HTMLButtonElement,
    );
    await waitFor(() => {
      const call = invoke.mock.calls.find(([cmd]) => cmd === 'signing_set_repo');
      if (!call) throw new Error('not saved');
      const overrides = (call[1] as { overrides: Record<string, unknown> }).overrides;
      expect(overrides['program']).toBe('/usr/local/bin/gpg2');
    });
  });

  it('still lists a key the program cannot see, rather than losing it', async () => {
    // Otherwise a key configured on another machine looks like it was forgotten.
    const { container } = pane(
      {
        effective: {
          format: 'openpgp',
          program: '',
          key: 'DEADBEEF00000000000000000000000000000000',
          signCommits: false,
          signTags: false,
        },
        local: {
          format: null,
          program: null,
          key: 'DEADBEEF00000000000000000000000000000000',
          signCommits: null,
          signTags: null,
        },
      },
      KEYS,
    );
    await waitFor(() => {
      const options = [...container.querySelectorAll('#key option')].map((o) => o.textContent);
      expect(options.some((o) => o?.includes('not in the keyring'))).toBe(true);
    });
  });

  it('warns about an expired key and offers a way to read about it', async () => {
    const { container } = pane({
      effective: { format: 'openpgp', program: '', key: OLD, signCommits: false, signTags: false },
      local: { format: null, program: null, key: OLD, signCommits: null, signTags: null },
    });
    await waitFor(() => {
      const warn = container.querySelector('.hint.warn');
      if (!warn) throw new Error('no warning');
      expect(warn.textContent).toContain('expired');
      expect(warn.querySelector('button')?.textContent).toContain('renew');
    });
  });

  it('offers key generation only where it makes sense', async () => {
    const { container } = pane();
    await waitFor(() => field(container, '#pass'));

    // ssh keys are made with ssh-keygen against a file, not generated into a keyring here.
    const fmt = field<HTMLSelectElement>(container, '#fmt');
    fmt.selectedIndex = [...fmt.options].findIndex((o) => o.value === 'ssh');
    await fireEvent.change(fmt);
    await waitFor(() => {
      if (container.querySelector('#pass')) throw new Error('still offering to generate');
    });
  });

  it('re-reads the keys when the format changes', async () => {
    const { container } = pane();
    await waitFor(() => field(container, '#fmt'));
    invoke.mockClear();
    await fireEvent.change(field<HTMLSelectElement>(container, '#fmt'));
    await waitFor(() => {
      if (!invoke.mock.calls.some(([cmd]) => cmd === 'signing_keys')) {
        throw new Error('keys not re-read');
      }
    });
  });
});
