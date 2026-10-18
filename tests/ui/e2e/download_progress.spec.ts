import { expect, test } from '@playwright/test';

test('пауза и продолжение сохраняют независимые прогрессы', async ({ page }) => {
  await page.goto('/tests/ui/harness/index.html');
  await page.getByRole('radio', { name: 'По ссылке', exact: true }).check();
  await page
    .getByLabel('Ссылка на словарь', { exact: true })
    .fill('https://example.org/dictionary.zip?token=e2e-secret');
  await page
    .getByLabel('Ссылки на ресурсы', { exact: true })
    .fill('https://example.org/resources.zip');
  await page.getByRole('button', { name: 'Добавить и начать' }).click();
  const download = page.getByRole('progressbar', { name: 'Скачивание', exact: true });
  const conversion = page.getByRole('progressbar', { name: 'Конвертация', exact: true });
  await expect(download).toHaveAttribute('aria-valuenow', '50');
  await expect(conversion).not.toHaveAttribute('aria-valuenow');
  await expect(page.getByRole('heading', { name: 'Результат', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Приостановить скачивание' }).click();
  await expect(page.getByText('На паузе', { exact: true })).toBeVisible();
  await expect(download).toHaveAttribute('aria-valuenow', '50');
  await page.getByRole('button', { name: 'Продолжить скачивание' }).click();
  await page.getByRole('button', { name: 'Шаг: конвертация', exact: true }).click();
  await expect(download).toHaveAttribute('aria-valuenow', '100');
  await expect(conversion).toHaveAttribute('aria-valuenow', '25');
  await expect(page.getByText('Запись MDX', { exact: true })).toBeVisible();
  await expect(page.getByText('Готово', { exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Шаг: готово', exact: true }).click();
  await page.getByRole('button', { name: 'Событие старого запуска', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Результат', exact: true })).toBeVisible();
  await expect(conversion).toHaveAttribute('aria-valuenow', '100');
  await expect(download).toHaveAttribute('aria-valuenow', '100');
});
test('один неизвестный размер убирает общий процент', async ({ page }) => {
  await page.goto('/tests/ui/harness/index.html?scenario=unknown');
  await expect(
    page.getByRole('progressbar', { name: 'Скачивание', exact: true }),
  ).not.toHaveAttribute('aria-valuenow');
  await expect(page.getByText('Общий размер неизвестен', { exact: true })).toBeVisible();
  await expect(page.getByText('8 ГиБ', { exact: true })).toBeVisible();
  await expect(page.getByText(/4 ГиБ из 8 ГиБ · 50%/)).toBeVisible();
});
