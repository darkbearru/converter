import { expect, test } from '@playwright/test';
import { screenUrl, step } from './helpers';

test('независимый словарь запускается отдельной задачей только после согласия', async ({
  page,
}) => {
  await page.goto(screenUrl('additional'));
  const dialog = page.getByRole('dialog', { name: 'Другие словари в архиве', exact: true });
  await expect(dialog).toBeVisible();
  await expect(dialog).toContainText('Universal (Ch-Ru)');
  await expect(dialog).toContainText('PhraseBookRuCh.dsl.dz');
  await expect(page.locator('.active-task')).toHaveCount(1);
  await dialog.getByRole('button', { name: 'Конвертировать', exact: true }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator('.active-task')).toHaveCount(2);
  await step(page, 'stale');
  await expect(dialog).toBeHidden();
  await expect(page.locator('.active-task')).toHaveCount(2);
  await page.goto(screenUrl('additional'));
  await expect(dialog).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  await expect(page.locator('.active-task')).toHaveCount(1);
});

test('ошибка решения позволяет повтор, ожидание блокирует повтор и Escape', async ({ page }) => {
  await page.goto(screenUrl('additional-error'));
  const dialog = page.getByRole('dialog', { name: 'Другие словари в архиве', exact: true });
  await dialog.getByRole('button', { name: 'Конвертировать', exact: true }).click();
  await expect(dialog.getByRole('alert')).toContainText('Не удалось сохранить очередь');
  await dialog.getByRole('button', { name: 'Конвертировать', exact: true }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator('.active-task')).toHaveCount(2);
  await page.goto(screenUrl('additional-busy'));
  await dialog.getByRole('button', { name: 'Конвертировать', exact: true }).click();
  await expect(dialog.getByRole('button', { name: 'Конвертировать', exact: true })).toBeDisabled();
  await expect(dialog.getByRole('button', { name: 'Пропустить', exact: true })).toBeDisabled();
  await page.keyboard.press('Escape');
  await expect(dialog).toBeVisible();
  await step(page, 'advance');
  await expect(dialog).toBeHidden();
  await expect(page.locator('.active-task')).toHaveCount(2);
});

test('предложение с длинным именем помещается в минимальное окно', async ({ page }) => {
  await page.setViewportSize({ width: 540, height: 400 });
  await page.goto(screenUrl('additional-long'));
  const dialog = page.getByRole('dialog', { name: 'Другие словари в архиве', exact: true });
  await expect(dialog.getByRole('button', { name: 'Пропустить', exact: true })).toBeInViewport({
    ratio: 1,
  });
  await expect(dialog.getByRole('button', { name: 'Конвертировать', exact: true })).toBeInViewport({
    ratio: 1,
  });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await dialog.getByRole('button', { name: 'Пропустить', exact: true }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator('.active-task')).toHaveCount(1);
});
