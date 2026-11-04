/**
 * Every string the translated surfaces show, in English, which is also what any language
 * without a string of its own falls back to.
 *
 * A message with forms is an object of `Intl.PluralRules` categories; `{name}` is filled in
 * from what the caller passes.
 */
export const en = {
  'toolbar.undo': 'Undo',
  'toolbar.redo': 'Redo',
  'toolbar.fetch': 'Fetch',
  'toolbar.pull': 'Pull',
  'toolbar.push': 'Push',
  'toolbar.branch': 'Branch',
  'toolbar.stash': 'Stash',
  'toolbar.pop': 'Pop',
  'toolbar.patch': 'Patch',
  'toolbar.leftPanel': 'Left panel',
  'toolbar.rightPanel': 'Right panel',
  'toolbar.terminal': 'Terminal',
  'toolbar.crumb.repository': 'repository',
  'toolbar.crumb.submodule': 'submodule',
  'toolbar.crumb.branch': 'branch',
  'toolbar.pullMenu': 'Choose how to pull',
  'toolbar.pushMenu': 'Choose what to push',
  'toolbar.backTo': 'Back to {repo}',

  'prefs.title': 'Preferences',
  'prefs.close': 'Close',
  'prefs.closeHint': 'Close preferences (Esc)',
  'prefs.pane.appearance': 'Appearance',
  'prefs.pane.profiles': 'Profiles',
  'prefs.pane.ssh': 'SSH',
  'prefs.pane.signing': 'Commit Signing',
  'prefs.pane.repository': 'Repository',
  'prefs.pane.terminal': 'Terminal',
  'prefs.pane.tools': 'Tools',
  'prefs.pane.experimental': 'Experimental',
  'prefs.pane.about': 'About',

  'appearance.language': 'Language',
  'appearance.language.system': 'As the desktop is set',
  'appearance.language.hint':
    'Only part of the window is translated so far; the rest stays in English until it is.',

  'staging.fileChanges': { one: '{n} file change', other: '{n} file changes' },
  'staging.noFileChanges': 'No file changes',
  'staging.on': 'on',
  'staging.path': 'Path',
  'staging.tree': 'Tree',
  'staging.unstaged': 'Unstaged files',
  'staging.staged': 'Staged files',
  'staging.stageAll': 'Stage all changes',
  'staging.unstageAll': 'Unstage all changes',
  'staging.expandAll': 'Expand all',
  'staging.collapseAll': 'Collapse all',
  'staging.nothingUnstaged': 'Nothing unstaged.',
  'staging.nothingStaged': 'Nothing staged. A commit needs something in here.',
  'staging.stageIt': 'Stage it',
  'staging.unstageIt': 'Unstage it',
  'staging.compose': 'Commit message',
  'staging.amend': 'Amend the previous commit',
  'staging.summary': 'Commit summary',
  'staging.description': 'Description',
  'staging.draft': 'Draft',
  'staging.drafting': 'Drafting…',
  'staging.needsStaged': 'Stage something to commit',
  'staging.needsSummary': 'Write a summary to commit',
  'staging.needsTemplate': 'Fill in the template to commit',
  'staging.commitFiles': { one: 'Commit {n} file', other: 'Commit {n} files' },

  'status.noToken': '· no token',
  'status.whatIsThis': '· what is this?',
  'status.commits': { one: '{n} commit', other: '{n} commits' },
  'status.changed': '{n} changed',
} as const;

export type MessageKey = keyof typeof en;

/** One message: plain, or with a form for each plural category the language uses. */
export type Message = string | Partial<Record<Intl.LDMLPluralRule, string>>;
