import { expect, test } from '@playwright/test';
import { screenUrl } from './helpers';

test('пароль ZIP повторяется после ошибки, затем запускается конвертация', async ({ page }) => {
  await page.goto(screenUrl('archive-password') + '&lang=en');
  const dialog = page.getByRole('dialog', { name: 'Archive password', exact: true });
  const password = dialog.getByLabel('Password', { exact: true });
  await expect(password).toBeFocused();
  await expect(password).toHaveAttribute('type', 'password');
  await password.fill('wrong');
  await dialog.getByRole('button', { name: 'Continue', exact: true }).click();
  await expect(dialog).toContainText('incorrect or the archive is damaged');
  await expect(password).toHaveValue('');
  await password.fill('secret');
  await password.press('Enter');
  await expect(dialog).toBeHidden();
  await expect(page.locator('.active-task')).toHaveCount(1);
});

test('отмена ввода ZIP сохраняет задачу и не сохраняет введённый пароль', async ({ page }) => {
  await page.goto(screenUrl('archive-password'));
  const dialog = page.getByRole('dialog', { name: 'Пароль архива', exact: true });
  await dialog.getByLabel('Пароль', { exact: true }).fill('private-value');
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  await page.locator('.p-accordionheader').click();
  await page.getByRole('button', { name: 'Уточнить параметры', exact: true }).click();
  await expect(dialog.getByLabel('Пароль', { exact: true })).toHaveValue('');
});
