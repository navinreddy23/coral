import { describe, expect, it } from 'vitest';
import { de } from '../src/i18n/de';
import { en } from '../src/i18n/en';
import { locale, resolveLanguage, t } from '../src/state/locale.svelte';

const holes = (s: unknown) =>
  [...(typeof s === 'string' ? s : Object.values(s as object).join(' ')).matchAll(/\{(\w+)\}/gu)]
    .map((m) => m[1])
    .sort();

describe('the catalogues', () => {
  it('translate only keys English has, with the same places to fill in', () => {
    for (const [key, message] of Object.entries(de)) {
      expect(key in en, key).toBe(true);
      expect(holes(message), key).toEqual(holes(en[key as keyof typeof en]));
    }
  });

  it('fills in values, picks plural forms by the language, and falls back to English', () => {
    locale.language = 'en';
    expect(t('staging.commitFiles', { n: 1 })).toBe('Commit 1 file');
    expect(t('staging.commitFiles', { n: 3 })).toBe('Commit 3 files');
    expect(t('toolbar.backTo', { repo: 'coral' })).toBe('Back to coral');
    locale.language = 'de';
    expect(t('staging.commitFiles', { n: 1 })).toBe('1 Datei committen');
    expect(t('staging.commitFiles', { n: 2 })).toBe('2 Dateien committen');
    expect(t('prefs.title')).toBe('Einstellungen');
    locale.language = 'en';
  });

  it('follows the desktop for "system", and English when it knows none of its languages', () => {
    expect(resolveLanguage('system', ['de-AT', 'en-GB'])).toBe('de');
    expect(resolveLanguage('system', ['fr-FR'])).toBe('en');
    expect(resolveLanguage('de', ['en-US'])).toBe('de');
    expect(resolveLanguage('xx', [])).toBe('en');
  });
});
