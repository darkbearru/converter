import { inject, reactive, type InjectionKey } from 'vue';
import { createI18n } from 'vue-i18n';
import type { NativeBridge } from '../contracts';
import type { AppPreferences, LanguagePreference } from './preferences';
import ru from '../../../locales/ru.json';
import en from '../../../locales/en.json';
export * from './preferences';
export const catalogs = { ru, en };
export function createLocalization(initial: AppPreferences, bridge: NativeBridge) {
  const preferences = reactive({ ...initial });
  const i18n = createI18n({
    legacy: false,
    locale: initial.locale,
    fallbackLocale: 'en',
    messages: catalogs,
    missingWarn: false,
    fallbackWarn: false,
    pluralRules: {
      ru: (choice: number, length: number) => {
        const value = Math.abs(choice);
        const mod100 = value % 100;
        const index =
          mod100 >= 11 && mod100 <= 14
            ? 2
            : value % 10 === 1
              ? 0
              : value % 10 >= 2 && value % 10 <= 4
                ? 1
                : 2;
        return Math.min(index, length - 1);
      },
    },
  });
  const context = {
    i18n,
    preferences,
    async setLanguage(language: LanguagePreference) {
      const saved = await bridge.setAppLanguage(language);
      Object.assign(preferences, saved);
      i18n.global.locale.value = saved.locale;
    },
  };
  current = context;
  return context;
}
export type Localization = ReturnType<typeof createLocalization>;
export const localizationKey: InjectionKey<Localization> = Symbol('localization');
let current: Localization | undefined;
export function useLocalization(): Localization {
  const result = inject(localizationKey, undefined) ?? current;
  if (!result) throw new Error('Localization was not initialized');
  return result;
}
export function activeLocale() {
  return current?.i18n.global.locale.value ?? 'en';
}
export function text(
  key: string,
  args: Record<string, string | number> = {},
  count?: number,
): string {
  if (!current) return key;
  return count === undefined
    ? current.i18n.global.t(key, args)
    : current.i18n.global.t(key, args, count);
}
