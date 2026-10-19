import { expect, test } from '@playwright/test';
import { screenUrl, step } from './helpers';

test('основной язык и ручной выбор переключают приложение без потери задачи', async ({ page }) => {
  await page.goto(screenUrl('stack') + '&lang=en');
  await expect(page.getByRole('button', { name: 'Support the developer' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Report an issue' })).toBeVisible();
  const before = await page.locator('.active-task').count();
  await step(page, 'about');
  const dialog = page.getByRole('dialog', { name: 'About Converter' });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole('link', { name: 'Say thanks' })).toHaveAttribute(
    'href',
    'https://hipolink.net/darkbear_ru/tips',
  );
  await dialog.getByRole('combobox', { name: 'Language' }).click();
  await page.getByRole('option', { name: 'Русский', exact: true }).click();
  await expect(page.getByRole('dialog', { name: 'О программе' })).toBeVisible();
  await expect(page.getByRole('combobox', { name: 'Язык' })).toBeFocused();
  await expect(page.getByText('Сказать спасибо')).toBeVisible();
  await page
    .getByRole('dialog', { name: 'О программе' })
    .locator('.p-dialog-footer')
    .getByRole('button', { name: 'Закрыть', exact: true })
    .click();
  await expect(page.locator('.active-task')).toHaveCount(before);
});

test('неподдерживаемый основной язык использует английский', async ({ page }) => {
  await page.goto(screenUrl('ready') + '&system=de-DE');
  await expect(page.getByRole('button', { name: 'Convert', exact: true })).toBeVisible();
  await step(page, 'about');
  const language = page
    .getByRole('dialog', { name: 'About Converter' })
    .getByRole('combobox', { name: 'Language' });
  await expect(language).toHaveText('System');
  await language.click();
  await expect(page.getByRole('option', { name: 'Русский', exact: true })).toBeVisible();
  await expect(page.getByRole('option', { name: 'English', exact: true })).toBeVisible();
});

test('смена языка сохраняет введённую ссылку и переводит новые предупреждения', async ({
  page,
}) => {
  await page.goto(screenUrl('localized-preview') + '&lang=en');
  await page.locator('.p-accordionheader').first().click();
  await expect(page.locator('.diagnostic-list-preview')).toContainText(
    'No entry found for link: sample',
  );
  await page.getByRole('button', { name: 'Add URL', exact: true }).click();
  const url = 'https://example.org/dictionary.zip';
  await page
    .getByRole('dialog', { name: 'Download dictionary', exact: true })
    .getByLabel('Dictionary URL')
    .fill(url);
  await step(page, 'about');
  const about = page.getByRole('dialog', { name: 'About Converter' });
  await about.getByRole('combobox', { name: 'Language' }).click();
  await page.getByRole('option', { name: 'Русский', exact: true }).click();
  await page
    .getByRole('dialog', { name: 'О программе' })
    .locator('.p-dialog-footer')
    .getByRole('button', { name: 'Закрыть', exact: true })
    .click();
  await expect(
    page
      .getByRole('dialog', { name: 'Скачать словарь', exact: true })
      .getByLabel('Ссылка на словарь'),
  ).toHaveValue(url);
  await expect(
    page
      .getByRole('dialog', { name: 'Скачать словарь', exact: true })
      .getByLabel('Ссылка на словарь'),
  ).toBeFocused();
  await page
    .getByRole('dialog', { name: 'Скачать словарь', exact: true })
    .getByRole('button', { name: 'Отмена', exact: true })
    .click();
  await expect(page.locator('.diagnostic-list-preview')).toContainText(
    'Не найдена статья для ссылки: sample',
  );
  await expect(page.locator('.history-file')).toHaveCount(2);
});
