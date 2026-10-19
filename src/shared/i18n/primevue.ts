import { watch } from 'vue';
import { catalogs, type Localization } from './index';
export function syncPrimeVue(localization: Localization, target: { locale?: unknown }) {
  return watch(
    () => localization.preferences.locale,
    (locale) => {
      target.locale = structuredClone(catalogs[locale].primevue);
    },
    { immediate: true },
  );
}
