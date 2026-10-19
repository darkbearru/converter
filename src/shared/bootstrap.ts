import { createApp } from 'vue';
import { usePrimeVue } from 'primevue/config';
import App from '../App.vue';
import type { NativeBridge } from './contracts';
import { PrimeVue, primeVueOptions } from './theme';
import {
  catalogs,
  createLocalization,
  localizationKey,
  preferencesFor,
  type AppPreferences,
} from './i18n';
import { syncPrimeVue } from './i18n/primevue';
export async function mountApplication(bridge: NativeBridge, initial?: AppPreferences) {
  const preferences =
    initial ?? (await bridge.getAppPreferences().catch(() => preferencesFor('system', null)));
  const localization = createLocalization(preferences, bridge);
  const app = createApp(App, { bridge });
  app
    .use(localization.i18n)
    .use(PrimeVue, {
      ...primeVueOptions,
      locale: structuredClone(catalogs[preferences.locale].primevue),
    });
  app.provide(localizationKey, localization);
  const primevue = app.runWithContext(usePrimeVue);
  const stop = syncPrimeVue(localization, primevue.config);
  app.onUnmount(stop);
  app.mount('#app');
  return app;
}
