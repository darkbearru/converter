import { expect, test } from '@playwright/test';
import { screenUrl } from './helpers';

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
  const inputBox = (await input.boundingBox())!;
  const pasteBox = (await paste.boundingBox())!;
  expect(pasteBox.x).toBeGreaterThan(inputBox.x);
  expect(pasteBox.x + pasteBox.width).toBeLessThan(inputBox.x + inputBox.width);
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
