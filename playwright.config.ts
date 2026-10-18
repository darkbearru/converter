import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests/ui/e2e',
  timeout: 20_000,
  expect: { timeout: 5_000 },
  fullyParallel: true,
  workers: process.env.CI ? 2 : 4,
  reporter: [['list'], ['html', { open: 'never', outputFolder: '.work/playwright-report' }]],
  outputDir: '.work/playwright-results',
  use: { baseURL: 'http://127.0.0.1:1421', trace: 'retain-on-failure' },
  projects: [
    {
      name: 'chromium-desktop',
      use: { browserName: 'chromium', viewport: { width: 740, height: 500 }, colorScheme: 'light' },
    },
    {
      name: 'webkit-desktop',
      use: { browserName: 'webkit', viewport: { width: 740, height: 500 }, colorScheme: 'light' },
    },
    {
      name: 'chromium-dark',
      use: { browserName: 'chromium', viewport: { width: 740, height: 500 }, colorScheme: 'dark' },
    },
    {
      name: 'webkit-dark',
      use: { browserName: 'webkit', viewport: { width: 740, height: 500 }, colorScheme: 'dark' },
    },
  ],
  webServer: {
    command: 'npm run dev:harness',
    url: 'http://127.0.0.1:1421',
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
  },
});
