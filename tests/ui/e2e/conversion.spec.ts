import { expect, test, type Locator, type Page } from '@playwright/test';

async function keyboardFocus(page: Page, target: Locator, key: string) {
  for (let count = 0; count < 40; count++) {
    await page.keyboard.press(key);
    if (await target.evaluate((el) => el === document.activeElement)) return;
  }
  await expect(target).toBeFocused();
}
test('локальный файл даёт три готовых файла', async ({ page }) => {
  await page.goto('/tests/ui/harness/index.html');
  await expect(page.getByTestId('harness-controls')).toBeVisible();
  await page.getByRole('button', { name: 'Выбрать словари', exact: true }).click();
  await expect(page.getByText('Словарь.dsl', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Добавить и начать' }).click();
  await expect(page.getByRole('heading', { name: 'Результат', exact: true })).toBeVisible();
  await expect(page.getByText('dictionary.mdx', { exact: true })).toBeVisible();
  await expect(page.getByText('dictionary.mdd', { exact: true })).toBeVisible();
  await expect(page.getByText('conversion-report.json', { exact: true })).toBeVisible();
  await expect(page.getByText('Локальный файл', { exact: true })).toBeVisible();
  await expect(page.getByRole('progressbar', { name: 'Скачивание' })).toHaveCount(0);
  await page.getByRole('button', { name: 'Сохранить копию', exact: true }).click();
  await expect(page.getByText('Копия сохранена. Файлов: 3.')).toBeVisible();
});
test('словарь выбирается и запускается только клавиатурой', async ({ page, browserName }) => {
  const key = browserName === 'webkit' && process.platform === 'darwin' ? 'Alt+Tab' : 'Tab';
  await page.goto('/tests/ui/harness/index.html');
  const pick = page.getByRole('button', { name: 'Выбрать словари', exact: true });
  await keyboardFocus(page, pick, key);
  await page.keyboard.press('Enter');
  await expect(page.getByText('Словарь.dsl', { exact: true })).toBeVisible();
  const submit = page.getByRole('button', { name: 'Добавить и начать' });
  await keyboardFocus(page, submit, key);
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'Результат', exact: true })).toBeVisible();
});
test('ожидающее задание принимает кодировку и выбор всех словарей', async ({ page }) => {
  await page.goto('/tests/ui/harness/index.html?scenario=awaiting');
  await expect(page.getByRole('heading', { name: 'Уточните параметры' })).toBeVisible();
  await page.getByLabel('Кодировка DSL').selectOption('windows-1251');
  await page.getByLabel('Словарь в архиве').selectOption('*');
  await page.getByRole('button', { name: 'Сохранить параметры и запустить' }).click();
  await expect(page.getByRole('heading', { name: 'Задания 2' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Результат', exact: true })).toBeVisible();
});
