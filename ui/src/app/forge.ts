import type { Host } from '../ipc/commands';

/**
 * What the window says about each kind of host: its name, its word for a request, where its
 * tokens are made and what one needs, and what a review there can do.
 *
 * In one place, so a sixth host is a row here rather than a ternary in eight components.
 */
export interface Forge {
  label: string;
  /** "Pull request" or "Merge request", as the host calls one. */
  request: string;
  requests: string;
  /** Where a token is made, on this instance. */
  tokenPage: (host: Host) => string;
  /** What the token has to be allowed to do, in the host's own words. */
  scope: string;
  /** What to paste, when it is not simply the token. */
  shape: string | null;
  /** Whether a review can ask for changes rather than only approve. */
  requestsChanges: boolean;
}

const FORGES: Record<Host['kind'], Forge> = {
  github: {
    label: 'GitHub',
    request: 'Pull request',
    requests: 'Pull requests',
    tokenPage: (h) => `${h.origin}/settings/tokens`,
    scope: 'repo',
    shape: null,
    requestsChanges: true,
  },
  gitlab: {
    label: 'GitLab',
    request: 'Merge request',
    requests: 'Merge requests',
    tokenPage: (h) => `${h.origin}/-/user_settings/personal_access_tokens`,
    scope: 'api',
    shape: null,
    // GitLab has no verdict short of approving for a merge request.
    requestsChanges: false,
  },
  gitea: {
    label: 'Gitea',
    request: 'Pull request',
    requests: 'Pull requests',
    tokenPage: (h) => `${h.origin}/user/settings/applications`,
    scope: 'repository: read and write',
    shape: null,
    requestsChanges: true,
  },
  bitbucket: {
    label: 'Bitbucket',
    request: 'Pull request',
    requests: 'Pull requests',
    tokenPage: (h) => `${h.origin}/${h.owner}/${h.repo}/admin/access-tokens`,
    scope: 'pull requests: write',
    shape: 'a repository access token, or an Atlassian API token written as email:token',
    requestsChanges: true,
  },
  azure: {
    label: 'Azure DevOps',
    request: 'Pull request',
    requests: 'Pull requests',
    tokenPage: (h) =>
      h.origin.endsWith('.visualstudio.com')
        ? `${h.origin}/_usersSettings/tokens`
        : `${h.origin}/${h.owner.split('/')[0] ?? ''}/_usersSettings/tokens`,
    scope: 'Code: read and write',
    shape: null,
    requestsChanges: true,
  },
};

/** The host's own words and pages, or GitHub's for a host not yet known. */
export function forgeOf(kind: Host['kind'] | undefined): Forge {
  return FORGES[kind ?? 'github'];
}
