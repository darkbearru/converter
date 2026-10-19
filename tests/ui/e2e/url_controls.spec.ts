import { expect, test } from '@playwright/test';
import { screenUrl } from './helpers';

for (const [language, expected] of [
  ['ru', '4 Гб из 8 Гб'],
  ['en', '4 GB of 8 GB'],
]) {
  test(`счётчик загрузки отделяет предлог пробелами: ${language}`, async ({ page }) => {
    await page.goto(screenUrl('downloading') + `&lang=${language}`);
    await expect(page.locator('.transfer-caption > span').last()).toHaveText(expected);
  });
}

test('ссылка получает текст буфера внутри поля; шапка сохраняет компактные размеры', async ({
  page,
  browserName,
}, testInfo) => {
  await page.goto(screenUrl());
  const support = page.getByRole('button', { name: 'Поддержать разработчика' });
  await expect(support).toBeVisible();
  expect(
    await support.evaluate((element) => {
      const rect = element.getBoundingClientRect();
      const icon = element.querySelector('.app-icon')!.getBoundingClientRect();
      return {
        width: rect.width,
        height: rect.height,
        iconWidth: icon.width,
        iconHeight: icon.height,
      };
    }),
  ).toEqual({ width: 24, height: 24, iconWidth: 16, iconHeight: 16 });
  await support.click();
  await expect(page.getByRole('alert')).toHaveCount(0);
  await page.getByRole('button', { name: 'Добавить ссылку', exact: true }).click();
  await expect(page.getByLabel('SHA-256 (необязательно)')).toHaveCount(0);
  const input = page.getByLabel('Ссылка на словарь', { exact: true });
  const paste = page.getByRole('button', { name: 'Вставить из буфера обмена' });
  // Оба размера снимаются в одном кадре: диалог ещё может масштабироваться при открытии.
  const placement = await input.evaluate((element) => {
    const field = element.getBoundingClientRect();
    const icon = element.parentElement!.querySelector('.paste-url')!.getBoundingClientRect();
    return { left: icon.left - field.left, right: field.right - icon.right };
  });
  expect(placement.left).toBeGreaterThan(0);
  expect(placement.right).toBeGreaterThan(0);
  await paste.click();
  await expect(input).toHaveValue('https://example.org/pasted.dsl');
  await expect(page.getByRole('dialog', { name: 'Скачать словарь' })).toBeVisible();
  await expect(page.getByRole('progressbar')).toHaveCount(0);
  await input.focus();
  await page.keyboard.press(
    browserName === 'webkit' && process.platform === 'darwin' ? 'Alt+Tab' : 'Tab',
  );
  await expect(paste).toBeFocused();
  await expect(paste).toHaveCSS('outline-style', 'solid');
  expect(
    await paste.evaluate((element) => parseFloat(getComputedStyle(element).outlineWidth)),
  ).toBeGreaterThan(0);
  await page.screenshot({ path: testInfo.outputPath('url-controls.png') });
  await page.setViewportSize({ width: 540, height: 400 });
  await expect(paste).toBeInViewport();
  await expect(page.getByRole('button', { name: 'Скачать', exact: true })).toBeInViewport();
  await page.screenshot({ path: testInfo.outputPath('url-controls-narrow.png') });
});
