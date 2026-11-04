import type { Message, MessageKey } from './en';

/**
 * German.
 *
 * Machine-written and not yet read by a native speaker; corrections are welcome and expected.
 * A key left out here shows in English, which is how a new string reaches the window before
 * it is translated.
 */
export const de: Partial<Record<MessageKey, Message>> = {
  'toolbar.undo': 'Rückgängig',
  'toolbar.redo': 'Wiederholen',
  'toolbar.fetch': 'Abrufen',
  'toolbar.pull': 'Pull',
  'toolbar.push': 'Push',
  'toolbar.branch': 'Branch',
  'toolbar.stash': 'Stash',
  'toolbar.pop': 'Pop',
  'toolbar.patch': 'Patch',
  'toolbar.leftPanel': 'Linke Leiste',
  'toolbar.rightPanel': 'Rechte Leiste',
  'toolbar.terminal': 'Terminal',
  'toolbar.crumb.repository': 'Repository',
  'toolbar.crumb.submodule': 'Submodul',
  'toolbar.crumb.branch': 'Branch',
  'toolbar.pullMenu': 'Wählen, wie gepullt wird',
  'toolbar.pushMenu': 'Wählen, was gepusht wird',
  'toolbar.backTo': 'Zurück zu {repo}',

  'prefs.title': 'Einstellungen',
  'prefs.close': 'Schließen',
  'prefs.closeHint': 'Einstellungen schließen (Esc)',
  'prefs.pane.appearance': 'Darstellung',
  'prefs.pane.profiles': 'Profile',
  'prefs.pane.ssh': 'SSH',
  'prefs.pane.signing': 'Commit-Signatur',
  'prefs.pane.repository': 'Repository',
  'prefs.pane.terminal': 'Terminal',
  'prefs.pane.tools': 'Werkzeuge',
  'prefs.pane.experimental': 'Experimentell',
  'prefs.pane.about': 'Über',

  'appearance.language': 'Sprache',
  'appearance.language.system': 'Wie am Desktop eingestellt',
  'appearance.language.hint':
    'Bisher ist nur ein Teil des Fensters übersetzt; der Rest bleibt englisch, bis er es ist.',

  'staging.on': 'auf',
  'staging.path': 'Pfad',
  'staging.tree': 'Baum',
  'staging.unstaged': 'Nicht vorgemerkte Dateien',
  'staging.staged': 'Vorgemerkte Dateien',
  'staging.stageAll': 'Alle Änderungen vormerken',
  'staging.unstageAll': 'Alle Vormerkungen aufheben',
  'staging.nothingUnstaged': 'Nichts, was nicht vorgemerkt ist.',
  'staging.nothingStaged': 'Nichts vorgemerkt. Ein Commit braucht hier etwas.',
  'staging.stageIt': 'Vormerken',
  'staging.unstageIt': 'Vormerkung aufheben',
  'staging.compose': 'Commit-Nachricht',
  'staging.amend': 'Letzten Commit ergänzen',
  'staging.summary': 'Zusammenfassung',
  'staging.description': 'Beschreibung',
  'staging.draft': 'Entwurf',
  'staging.drafting': 'Entwurf entsteht…',
  'staging.needsStaged': 'Zum Committen etwas vormerken',
  'staging.needsSummary': 'Zum Committen eine Zusammenfassung schreiben',
  'staging.needsTemplate': 'Zum Committen die Vorlage ausfüllen',
  'staging.commitFiles': { one: '{n} Datei committen', other: '{n} Dateien committen' },

  'status.noToken': '· kein Token',
  'status.whatIsThis': '· was ist das?',
  'status.commits': { one: '{n} Commit', other: '{n} Commits' },
};
