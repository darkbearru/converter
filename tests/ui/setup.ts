import '@testing-library/jest-dom/vitest';
process.env.DEBUG_PRINT_LIMIT = '1600';
import { cleanup } from '@testing-library/vue';
import { afterEach, beforeEach } from 'vitest';
import { config } from '@vue/test-utils';
import PrimeVue from 'primevue/config';
import { primeVueOptions } from '../../src/shared/theme';
import {
  createLocalization,
  localizationKey,
  preferencesFor,
  catalogs,
} from '../../src/shared/i18n';
import { createHarness } from './harness/bridge';
beforeEach(() => {
  const localization = createLocalization(
    preferencesFor('ru', null),
    createHarness('local').bridge,
  );
  config.global.plugins = [
    localization.i18n,
    [PrimeVue, { ...primeVueOptions, locale: catalogs.ru.primevue }],
  ];
  config.global.provide = { [localizationKey as symbol]: localization };
});
Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: (query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener() {},
    removeEventListener() {},
    addListener() {},
    removeListener() {},
    dispatchEvent() {
      return true;
    },
  }),
});

afterEach(() => cleanup());
