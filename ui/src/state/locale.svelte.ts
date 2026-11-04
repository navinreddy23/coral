import { de } from '../i18n/de';
import { en, type Message, type MessageKey } from '../i18n/en';

/** The languages there is a catalogue for. */
export const LANGUAGES = [
  { id: 'en', name: 'English' },
  { id: 'de', name: 'Deutsch' },
] as const;

export type Language = (typeof LANGUAGES)[number]['id'];

const CATALOGUES: Record<Language, Partial<Record<MessageKey, Message>>> = { en, de };

/** The window's language, which every `t` call reads, so changing it redraws what uses it. */
class Locale {
  language = $state<Language>('en');
}

export const locale = new Locale();

/**
 * The language to show for a choice: the one chosen, or for `system` the first of the
 * desktop's languages there is a catalogue for, else English.
 */
export function resolveLanguage(choice: string, preferred: readonly string[]): Language {
  const known = (code: string) => LANGUAGES.find((l) => l.id === code.toLowerCase().split('-')[0])?.id;
  if (choice !== 'system') return known(choice) ?? 'en';
  for (const code of preferred) {
    const found = known(code);
    if (found !== undefined) return found;
  }
  return 'en';
}

/**
 * One message in the window's language, with `{name}` filled in from `values`. A message with
 * plural forms picks its form from `values.n` by the language's own rules.
 */
export function t(key: MessageKey, values: Record<string, string | number> = {}): string {
  const lang = locale.language;
  const message = CATALOGUES[lang][key] ?? en[key];
  let text: string;
  if (typeof message === 'string') {
    text = message;
  } else {
    type Forms = Partial<Record<Intl.LDMLPluralRule, string>>;
    const forms = message as Forms;
    const english = en[key] as Forms;
    const form = new Intl.PluralRules(lang).select(Number(values['n'] ?? 0));
    text = forms[form] ?? forms.other ?? english[form] ?? english.other ?? key;
  }
  return text.replace(/\{(\w+)\}/gu, (whole, name: string) =>
    name in values ? String(values[name]) : whole,
  );
}
