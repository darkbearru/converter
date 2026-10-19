import type { Page } from '@playwright/test';
export async function step(page: Page, id: 'advance' | 'finish' | 'stale' | 'about') {
  await page.evaluate((name) => document.getElementById(name)?.click(), id);
}
export const screenUrl = (scenario = '') =>
  `/tests/ui/harness/index.html?capture=1&scenario=${scenario}`;
