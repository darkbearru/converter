import { expect, test } from '@playwright/test';
import { screenUrl, step } from './helpers';
test('ссылка сначала скачивается с паузой, затем ждёт отдельного запуска', async ({ page }) => {
  await page.goto(screenUrl());
  await page.getByRole('button', { name: 'Добавить ссылку', exact: true }).click();
  await page
    .getByLabel('Ссылка на словарь', { exact: true })
    .fill('https://example.org/main.dsl?token=test-private');
  await page.getByRole('button', { name: 'Скачать', exact: true }).click();
  const download = page.getByRole('progressbar', { name: 'Скачивание', exact: true });
  await expect(download).toHaveAttribute('aria-valuenow', '50');
  await expect
    .poll(() => download.evaluate((el) => el.getBoundingClientRect().height))
    .toBeGreaterThanOrEqual(20);
  await expect(page.locator('.active-task')).toHaveCount(0);
  await page.getByRole('button', { name: 'Приостановить скачивание', exact: true }).click();
  await expect(page.getByText('На паузе', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Продолжить скачивание', exact: true }).click();
  await step(page, 'advance');
  await expect(page.getByRole('button', { name: 'Конвертировать', exact: true })).toBeVisible();
  await expect(page.locator('.active-task')).toHaveCount(0);
  await page.getByRole('button', { name: 'Конвертировать', exact: true }).click();
  await expect(page.locator('.active-task')).toHaveCount(1);
  await expect(page.getByRole('button', { name: 'Выбрать словари', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'main.dsl', exact: true }).click();
  await expect(page.getByRole('progressbar', { name: 'Скачивание', exact: true })).toHaveAttribute(
    'aria-valuenow',
    '100',
  );
  await expect(page.getByRole('progressbar', { name: 'Конвертация', exact: true })).toHaveAttribute(
    'aria-valuenow',
    '25',
  );
});
test('сбой загрузки ресурсов остаётся в стопке с продолжением', async ({ page }) => {
  await page.goto(screenUrl('resource-failed'));
  await expect(page.locator('.active-task')).toHaveCount(1);
  await page.getByRole('button', { name: 'Продолжить скачивание main.dsl', exact: true }).click();
  await expect(
    page.getByRole('button', { name: 'Приостановить скачивание main.dsl', exact: true }),
  ).toBeVisible();
});
test('неизвестный размер не превращается в нулевой процент', async ({ page }) => {
  await page.goto(screenUrl('unknown'));
  await expect(
    page.getByRole('progressbar', { name: 'Скачивание', exact: true }),
  ).not.toHaveAttribute('aria-valuenow');
  await expect(page.getByText('Общий размер неизвестен', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Отменить скачивание', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Выбрать словари', exact: true })).toBeVisible();
});
