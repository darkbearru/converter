import { watch } from 'vue';
import { catalogs, type Localization } from './index';
export function syncPrimeVue(localization: Localization, target: { locale?: unknown }) {
  const defaults = (target.locale ?? {}) as Record<string, unknown>;
  const defaultAria = (defaults.aria ?? {}) as Record<string, unknown>;
  return watch(
    () => localization.preferences.locale,
    (locale) => {
      const translated = structuredClone(catalogs[locale].primevue);
      target.locale = { ...defaults, ...translated, aria: { ...defaultAria, ...translated.aria } };
    },
    { immediate: true },
  );
}
