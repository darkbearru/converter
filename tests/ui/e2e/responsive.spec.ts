import { expect, test } from '@playwright/test';
import { screenUrl } from './helpers';
test('длинное имя, журнал и системная тема работают в изменяемом окне', async ({ page }) => {
  await page.goto(screenUrl('long'));
  const before = await page.locator('body').evaluate((el) => getComputedStyle(el).backgroundColor);
  const current = await page.evaluate(() => matchMedia('(prefers-color-scheme: dark)').matches);
  await page.emulateMedia({ colorScheme: current ? 'light' : 'dark' });
  await expect
    .poll(() => page.locator('body').evaluate((el) => getComputedStyle(el).backgroundColor))
    .not.toBe(before);
  await page.setViewportSize({ width: 540, height: 400 });
  await page.locator('.p-accordionheader').click();
  await page.getByRole('button', { name: /Журнал · 70/ }).click();
  const log = page.locator('.diagnostic-list');
  await expect(log.locator('li')).toHaveCount(50);
  await expect(log.locator('img')).toHaveCount(0);
  await page.getByRole('button', { name: 'Далее', exact: true }).click();
  await expect(log.locator('li')).toHaveCount(20);
  await expect
    .poll(() => page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
    .toBe(true);
  await expect(page.getByRole('button', { name: 'Выбрать словари', exact: true })).toBeVisible();
});
test('новые задачи образуют стопку, отмена и reduced motion сохраняют действия', async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto(screenUrl('stack'));
  await expect(page.locator('.active-task')).toHaveCount(2);
  for (let i = 0; i < 3; i++) {
    await page.getByRole('button', { name: 'Выбрать словари', exact: true }).click();
    await page.getByRole('button', { name: 'Конвертировать', exact: true }).click();
  }
  await expect(page.locator('.active-task')).toHaveCount(5);
  await expect
    .poll(() =>
      page
        .locator('.task-stack')
        .evaluate((el) => el.scrollHeight - el.clientHeight - el.scrollTop),
    )
    .toBeLessThan(2);
  await expect
    .poll(() =>
      page
        .locator('.active-task')
        .last()
        .evaluate((el) => getComputedStyle(el).transform),
    )
    .toBe('none');
  await page
    .locator('.active-task')
    .last()
    .getByRole('button', { name: 'Отменить main.dsl', exact: true })
    .click();
  await expect(page.locator('.active-task')).toHaveCount(4);
  await page.locator('.p-accordionheader').click();
  await expect(page.getByRole('button', { name: 'Запустить заново', exact: true })).toBeVisible();
});
