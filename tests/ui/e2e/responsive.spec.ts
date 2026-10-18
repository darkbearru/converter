import { expect, test } from '@playwright/test';

test('длинные названия и журнал остаются в пределах экрана', async ({ page }) => {
  await page.goto('/tests/ui/harness/index.html?scenario=long');
  await expect(page.getByRole('heading', { name: 'Результат', exact: true })).toBeVisible();
  await expect
    .poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth))
    .toBe(true);
  await page.getByRole('button', { name: /Журнал · 70/ }).click();
  const log = page.locator('.diagnostic-list');
  await expect(log.locator('li')).toHaveCount(50);
  await expect(log).toContainText('<img src=x onerror=alert(1)>');
  await expect(log.locator('img')).toHaveCount(0);
  await page.getByRole('button', { name: 'Далее', exact: true }).click();
  await expect(log.locator('li')).toHaveCount(20);
  await expect(log).toContainText('Предупреждение 51');
  await page.getByRole('button', { name: 'Назад', exact: true }).click();
  await expect(log.locator('li')).toHaveCount(50);
});
test('крупный текст и reduced motion сохраняют доступные действия', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/tests/ui/harness/index.html?scenario=converting');
  await page.addStyleTag({
    content:
      'body { font-size: 22px !important; } button, input, textarea, select { font-size: inherit !important; }',
  });
  await expect(page.getByRole('button', { name: 'Отменить задание', exact: true })).toBeVisible();
  await expect
    .poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth))
    .toBe(true);
  await expect(page.getByRole('button', { name: 'Отменить задание', exact: true })).toHaveCSS(
    'transition-duration',
    '0s',
  );
  await page.getByRole('button', { name: 'Отменить задание', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Запустить заново', exact: true })).toBeVisible();
});
