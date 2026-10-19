import config from '../../../config/locales.json';
export type Locale = 'ru' | 'en';
export type LanguagePreference = 'system' | Locale;
export interface AppPreferences {
  language: LanguagePreference;
  locale: Locale;
  support_url: string | null;
}
export function resolveLocale(language: LanguagePreference, system: string | null): Locale {
  if (language !== 'system') return language;
  return system?.split(/[-_]/)[0].toLowerCase() === 'ru' ? 'ru' : 'en';
}
export function preferencesFor(
  language: LanguagePreference,
  system: string | null,
): AppPreferences {
  const locale = resolveLocale(language, system);
  return { language, locale, support_url: config.locales[locale].support_url };
}
