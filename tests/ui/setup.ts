import '@testing-library/jest-dom/vitest';
process.env.DEBUG_PRINT_LIMIT = '1600';
import { cleanup } from '@testing-library/vue';
import { afterEach } from 'vitest';
import { config } from '@vue/test-utils';
import PrimeVue from 'primevue/config';
import { primeVueOptions } from '../../src/shared/theme';
config.global.plugins = [[PrimeVue, { locale: primeVueOptions.locale }]];
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
