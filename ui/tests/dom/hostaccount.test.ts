// @vitest-environment happy-dom
import { cleanup, render } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import { afterEach, describe, expect, it } from 'vitest';

import HostAccount from '../../src/app/HostAccount.svelte';
import type { HostView, TokenSource } from '../../src/ipc/commands';

afterEach(cleanup);

function view(token: TokenSource, kind: 'github' | 'gitlab' = 'github'): HostView {
  return {
    host: {
      kind,
      origin: kind === 'gitlab' ? 'https://gitlab.example' : 'https://github.com',
      owner: 'team',
      repo: 'thing',
    },
    detail: null,
    token,
  };
}

function dialog(over: Record<string, unknown> = {}) {
  const signedIn: string[] = [];
  let signedOut = 0;
  let closed = 0;
  const result = render(HostAccount, {
    props: {
      view: view('none'),
      profile: 'Work',
      shared: false,
      error: null,
      onSignIn: (t: string) => signedIn.push(t),
      onSignOut: () => (signedOut += 1),
      onClose: () => (closed += 1),
      ...over,
    },
  });
  return {
    container: result.container,
    signedIn,
    out: () => signedOut,
    closed: () => closed,
  };
}

function field(container: Element): HTMLInputElement {
  const input = container.querySelector('input[type="password"]');
  if (input === null) throw new Error('no token field');
  return input as HTMLInputElement;
}

describe('the host account dialog', () => {
  it('says a profile owns its own token, by name', () => {
    const { container } = dialog({ view: view('profile'), profile: 'Work' });
    const state = container.querySelector('.state')?.textContent ?? '';
    expect(state).toContain('Signed in as Work');
    expect(state).toContain('No other profile can see');
  });

  it('says when the token in use is the shared one', () => {
    // The distinction that matters: signing out of this one signs every profile out.
    const { container } = dialog({ view: view('shared'), shared: true });
    expect(container.querySelector('.state')?.textContent).toContain('every profile');
    expect(container.querySelector('.out')?.textContent?.trim()).toBe('Sign out everywhere');
  });

  it('names the profile on the sign-out button when the token is its own', () => {
    const { container } = dialog({ view: view('profile'), profile: 'Personal' });
    expect(container.querySelector('.out')?.textContent?.trim()).toBe('Sign Personal out');
  });

  it('offers no way to sign out when there is nothing stored', () => {
    const { container } = dialog();
    expect(container.querySelector('.out')).toBeNull();
  });

  it('will not send an empty token', async () => {
    const { container, signedIn } = dialog();
    const button = container.querySelector('.primary') as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    await fireEvent.input(field(container), { target: { value: '   ' } });
    expect(button.disabled).toBe(true);
    expect(signedIn).toEqual([]);
  });

  it('trims what is pasted and clears the field behind it', async () => {
    // A pasted token often carries a trailing newline, and a token left in the field is a
    // secret sitting on screen after the dialog has done its job.
    const { container, signedIn } = dialog();
    const input = field(container);
    await fireEvent.input(input, { target: { value: '  ghp_secret\n' } });
    await fireEvent.click(container.querySelector('.primary') as HTMLButtonElement);
    expect(signedIn).toEqual(['ghp_secret']);
    expect(input.value).toBe('');
  });

  it('signs in on Enter', async () => {
    const { container, signedIn } = dialog();
    const input = field(container);
    await fireEvent.input(input, { target: { value: 'glpat-secret' } });
    await fireEvent.keyDown(input, { key: 'Enter' });
    expect(signedIn).toEqual(['glpat-secret']);
  });

  it('points at the instance the remote is on, not the public one', () => {
    const { container } = dialog({ view: view('none', 'gitlab') });
    const link = container.querySelector('.hint a');
    expect(link?.getAttribute('href')).toBe(
      'https://gitlab.example/-/user_settings/personal_access_tokens',
    );
    expect(container.querySelector('.hint code')?.textContent).toBe('api');
  });

  it('asks GitHub for the scope GitHub calls it', () => {
    const { container } = dialog();
    expect(container.querySelector('.hint a')?.getAttribute('href')).toBe(
      'https://github.com/settings/tokens',
    );
    expect(container.querySelector('.hint code')?.textContent).toBe('repo');
  });

  it('keeps the token out of sight while it is typed', () => {
    // Not a styling choice: a token in a plain field is readable over a shoulder and ends up
    // in every screenshot of the window.
    const { container } = dialog();
    expect(field(container).getAttribute('type')).toBe('password');
  });

  it('shows a refusal from the host rather than closing on it', () => {
    const { container, closed } = dialog({ error: 'the host refused the request (401)' });
    expect(container.querySelector('.bad')?.textContent).toContain('401');
    expect(closed()).toBe(0);
  });

  it('closes on Escape and on the scrim', async () => {
    const { container, closed } = dialog();
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(closed()).toBe(1);
    await fireEvent.click(container.querySelector('.scrim') as HTMLElement);
    expect(closed()).toBe(2);
  });

  it('stays open when the panel itself is clicked', async () => {
    const { container, closed } = dialog();
    await fireEvent.click(container.querySelector('.panel') as HTMLElement);
    expect(closed()).toBe(0);
  });
});
