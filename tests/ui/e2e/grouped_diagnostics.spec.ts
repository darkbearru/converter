import { expect, test } from '@playwright/test';
import { screenUrl } from './helpers';

test('повторения не увеличивают число пунктов в ссылке и полном журнале', async ({ page }) => {
  await page.goto(screenUrl('grouped-warnings'));
  await page.locator('.p-accordionheader').click();
  await expect(page.getByText(/Встречается 1\s234 раза/)).toBeVisible();
  const link = page.getByRole('link', { name: '(ещё 1 предупреждение)' });
  await link.click();
  const dialog = page.getByRole('dialog', { name: 'Предупреждения', exact: true });
  await expect(dialog.locator('.diagnostic-list > li')).toHaveCount(2);
  await expect(dialog.getByText(/Встречается 1\s234 раза/)).toBeVisible();
  await expect(dialog.getByText('Не найдено сокращение: rare.')).toBeVisible();
  await expect(dialog.getByText(/Первое упоминание: Словарь.dsl, строка 10/)).toBeVisible();
  await expect(dialog.locator('.p-paginator')).toHaveCount(0);
  await dialog.getByRole('button', { name: 'Закрыть', exact: true }).last().click();
  await expect(dialog).toBeHidden();
  await expect(link).toBeFocused();
});
