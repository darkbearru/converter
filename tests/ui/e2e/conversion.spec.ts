import { expect, test } from '@playwright/test';
import { screenUrl, step } from './helpers';
test('выбранный файл ожидает запуска, затем попадает в историю с активным путём', async ({
  page,
}) => {
  await page.goto(screenUrl());
  await page.getByRole('button', { name: 'Выбрать словари', exact: true }).click();
  await expect(page.getByText('25 Мб', { exact: true })).toBeVisible();
  await expect(page.locator('.active-task')).toHaveCount(0);
  await page.getByRole('button', { name: 'Настройки конвертации', exact: true }).click();
  const colors = await page.getByRole('dialog').evaluate((dialog) => {
    const filled = getComputedStyle(
      [...dialog.querySelectorAll('button')].find((button) => button.textContent === 'Готово')!,
    );
    const outlined = getComputedStyle(
      [...dialog.querySelectorAll('button')].find(
        (button) => button.textContent === 'Выбрать папку',
      )!,
    );
    const luminance = (rgb: string) =>
      rgb
        .match(/[\d.]+/g)!
        .slice(0, 3)
        .map(Number)
        .map((channel) => {
          const v = channel / 255;
          return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
        })
        .reduce((sum, value, index) => sum + value * [0.2126, 0.7152, 0.0722][index]!, 0);
    const ratio = (a: string, b: string) => {
      const [x, y] = [luminance(a), luminance(b)].sort((x, y) => y - x);
      return (x! + 0.05) / (y! + 0.05);
    };
    return {
      filled: ratio(filled.color, filled.backgroundColor),
      outlined: ratio(outlined.color, getComputedStyle(dialog).backgroundColor),
    };
  });
  expect(colors.filled).toBeGreaterThanOrEqual(4.5);
  expect(colors.outlined).toBeGreaterThanOrEqual(4.5);
  await page.getByRole('combobox', { name: 'Кодировка DSL' }).click();
  await page.getByRole('option', { name: 'Windows-1251', exact: true }).click();
  await page.getByRole('button', { name: 'Выбрать папку', exact: true }).click();
  await expect(page.getByText('/selected/output', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Готово', exact: true }).click();
  await page.getByRole('button', { name: 'Конвертировать', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Выбрать словари', exact: true })).toBeVisible();
  await expect(page.locator('.active-task')).toHaveCount(1);
  await step(page, 'finish');
  await step(page, 'stale');
  await expect(page.locator('.active-task')).toHaveCount(0);
  await page.locator('.p-accordionheader').click();
  await expect(page.getByText('dictionary.mdx', { exact: true })).toBeVisible();
  await expect(page.getByText('dictionary.mdd', { exact: true })).toBeVisible();
  await page
    .getByRole('button', { name: 'Открыть папку /Словари/Англо-русский-converted', exact: true })
    .click();
  await page.getByRole('button', { name: 'Сохранить копию', exact: true }).click();
  await expect(page.getByText('Копия сохранена. Файлов: 3.', { exact: true })).toBeVisible();
});
test('нижняя панель доступна с клавиатуры', async ({ page, browserName }) => {
  await page.goto(screenUrl());
  const key = browserName === 'webkit' && process.platform === 'darwin' ? 'Alt+Tab' : 'Tab';
  const choose = page.getByRole('button', { name: 'Выбрать словари', exact: true });
  for (let i = 0; i < 18 && !(await choose.evaluate((el) => document.activeElement === el)); i++)
    await page.keyboard.press(key);
  await expect(choose).toBeFocused();
  await page.keyboard.press('Enter');
  const convert = page.getByRole('button', { name: 'Конвертировать', exact: true });
  for (let i = 0; i < 18 && !(await convert.evaluate((el) => document.activeElement === el)); i++)
    await page.keyboard.press(key);
  await expect(convert).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.locator('.active-task')).toHaveCount(1);
});
test('параметры остановленного задания сохраняют выбор словарей архива', async ({ page }) => {
  await page.goto(screenUrl('awaiting'));
  await page.locator('.p-accordionheader').click();
  await page.getByRole('button', { name: 'Уточнить параметры', exact: true }).click();
  await page.getByLabel('Кодировка DSL', { exact: true }).selectOption('windows-1251');
  await page.getByLabel('Словарь в архиве', { exact: true }).selectOption('*');
  await page.getByRole('button', { name: 'Сохранить параметры и запустить', exact: true }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(page.locator('.active-task')).toHaveCount(2);
});
