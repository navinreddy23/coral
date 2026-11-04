import { describe, expect, it } from 'vitest';
import { forgeOf } from '../src/app/forge';
import { quoteRequest } from '../src/app/request';
import type { Host } from '../src/ipc/commands';

const at = (kind: Host['kind'], origin: string, owner = 'team', repo = 'thing'): Host => ({
  kind,
  origin,
  owner,
  repo,
});

describe('what each host is called and where its tokens are made', () => {
  it('sends each to its own token page, on its own instance', () => {
    expect(forgeOf('gitea').tokenPage(at('gitea', 'https://git.example.invalid'))).toBe(
      'https://git.example.invalid/user/settings/applications',
    );
    expect(forgeOf('bitbucket').tokenPage(at('bitbucket', 'https://bitbucket.org'))).toBe(
      'https://bitbucket.org/team/thing/admin/access-tokens',
    );
    expect(
      forgeOf('azure').tokenPage(at('azure', 'https://dev.azure.com', 'acme/Platform', 'coral')),
    ).toBe('https://dev.azure.com/acme/_usersSettings/tokens');
    expect(
      forgeOf('azure').tokenPage(at('azure', 'https://acme.visualstudio.com', 'Platform', 'coral')),
    ).toBe('https://acme.visualstudio.com/_usersSettings/tokens');
  });

  it("uses each host's word for a request and its way of quoting one", () => {
    expect(forgeOf('gitlab').request).toBe('Merge request');
    expect(forgeOf('azure').request).toBe('Pull request');
    expect(forgeOf('gitlab').requestsChanges).toBe(false);
    expect(forgeOf('bitbucket').requestsChanges).toBe(true);
    expect(forgeOf(undefined).label).toBe('GitHub');
    expect(quoteRequest('azure', 12)).toBe('!12');
    expect(quoteRequest('gitea', 12)).toBe('#12');
    // Bitbucket wants to be told whose an API token is.
    expect(forgeOf('bitbucket').shape).toContain('email:token');
  });
});
