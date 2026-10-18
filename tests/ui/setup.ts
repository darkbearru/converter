import '@testing-library/jest-dom/vitest';
process.env.DEBUG_PRINT_LIMIT = '1600';
import { cleanup } from '@testing-library/vue';
import { afterEach } from 'vitest';

afterEach(() => cleanup());
