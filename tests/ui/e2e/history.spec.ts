import { expect, test } from '@playwright/test';
import { screenUrl } from './helpers';
import type { Locator } from '@playwright/test';

async function contrast(button: Locator) {
  return button.evaluate((el) => {
    const luminance = (rgb: string) =>
      rgb
        .match(/[\d.]+/g)!
        .slice(0, 3)
        .map(Number)
        .map((c) => {
          const v = c / 255;
          return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
        })
        .reduce((sum, v, i) => sum + v * [0.2126, 0.7152, 0.0722][i]!, 0);
    let background: Element | null = el;
    while (
      background &&
      ['transparent', 'rgba(0, 0, 0, 0)'].includes(getComputedStyle(background).backgroundColor)
    )
      background = background.parentElement;
    const values = [
      luminance(getComputedStyle(el).color),
      luminance(getComputedStyle(background!).backgroundColor),
    ].sort((a, b) => b - a);
    return (values[0]! + 0.05) / (values[1]! + 0.05);
  });
}

test('ссылка с пунктирным подчёркиванием открывает полный журнал и возвращает фокус', async ({
  page,
}) => {
  await page.goto(screenUrl('warning'));
  await page.locator('.p-accordionheader').click();
  const link = page.getByRole('link', { name: /ещё 28\s172 предупреждения/ });
  await expect(link).toBeVisible();
  expect(
    await link.evaluate((el) => ({
      tag: el.tagName,
      line: getComputedStyle(el).textDecorationLine,
      style: getComputedStyle(el).textDecorationStyle,
    })),
  ).toEqual({ tag: 'A', line: 'underline', style: 'dotted' });
  const originalUrl = page.url();
  await link.focus();
  await page.keyboard.press('Enter');
  const dialog = page.getByRole('dialog', { name: 'Предупреждения', exact: true });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText('PhoneticListChCh.dsl.dz', { exact: true })).toBeVisible();
  const rows = dialog.locator('.diagnostic-list li');
  await expect(rows).toHaveCount(50);
  await expect(dialog.locator('.p-paginator-current')).toHaveText('1–50 из 28173');
  await dialog.getByRole('button', { name: 'Последняя страница', exact: true }).click();
  await expect(rows).toHaveCount(23);
  await expect(dialog.locator('.p-paginator-current')).toHaveText('28151–28173 из 28173');
  await expect(rows.last()).toContainText('строка 28182');
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  await expect(link).toBeFocused();
  expect(page.url()).toBe(originalUrl);
  await page.setViewportSize({ width: 540, height: 400 });
  await link.click();
  await expect(dialog.locator('.p-paginator-current')).toHaveText('1–50 из 28173');
  await expect(rows).toHaveCount(50);
  await expect(
    dialog.getByRole('button', { name: 'Последняя страница', exact: true }),
  ).toBeInViewport({ ratio: 1 });
  await expect(dialog.getByRole('button', { name: 'Закрыть', exact: true }).last()).toBeInViewport({
    ratio: 1,
  });
  expect(
    await dialog
      .locator('.diagnostic-list')
      .evaluate((el) => el.clientHeight > 40 && el.scrollHeight > el.clientHeight),
  ).toBe(true);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await dialog.getByRole('button', { name: 'Закрыть', exact: true }).last().click();
  await expect(dialog).toBeHidden();
  await expect(link).toBeFocused();
});

test('предупреждение видно сразу, четыре действия помещаются в строку, удаление подтверждается', async ({
  page,
}) => {
  await page.goto(screenUrl('warning'));
  const tools = await page.locator('.composer-tools').evaluate((el) => {
    const buttons = [...el.querySelectorAll('button')];
    return {
      gap: parseFloat(getComputedStyle(el).gap),
      items: buttons.map((button) => {
        const icon = button.querySelector('.app-icon')!;
        const r = icon.getBoundingClientRect();
        return {
          width: r.width,
          height: r.height,
          border: getComputedStyle(button).borderTopWidth,
        };
      }),
    };
  });
  expect(tools.gap).toBe(12);
  expect(tools.items).toEqual([
    { width: 24, height: 24, border: '0px' },
    { width: 24, height: 24, border: '0px' },
  ]);
  await page.locator('.p-accordionheader').click();
  await expect(page.getByText('Не найдена статья для ссылки: 儿', { exact: true })).toBeVisible();
  for (const width of [740, 540]) {
    await page.setViewportSize({ width, height: 500 });
    const buttons = page
      .getByRole('group', { name: 'Действия с конвертацией' })
      .getByRole('button');
    await expect(buttons).toHaveCount(4);
    await expect(buttons).toHaveText(['Открыть папку', 'Сохранить копию', 'Повторить', 'Удалить']);
    const colors = await buttons.evaluateAll((els) =>
      els.map((el) =>
        getComputedStyle(el)
          .color.match(/[\d.]+/g)!
          .slice(0, 3)
          .map(Number),
      ),
    );
    for (const channels of colors.slice(0, 3))
      expect(Math.max(...channels) - Math.min(...channels)).toBeLessThanOrEqual(16);
    expect(colors[3]![0]! - colors[3]![1]!).toBeGreaterThan(40);
    const rects = await buttons.evaluateAll((els) =>
      els.map((el) => {
        const r = el.getBoundingClientRect();
        return { top: r.top, right: r.right };
      }),
    );
    expect(
      Math.max(...rects.map((r) => r.top)) - Math.min(...rects.map((r) => r.top)),
    ).toBeLessThan(1);
    expect(Math.max(...rects.map((r) => r.right))).toBeLessThanOrEqual(width - 8);
  }
  const deleteButton = page.getByRole('button', { name: 'Удалить', exact: true });
  expect(await contrast(deleteButton)).toBeGreaterThanOrEqual(4.5);
  await deleteButton.click();
  const dialog = page.getByRole('dialog');
  await expect(dialog).toContainText('Вы действительно хотите удалить');
  await expect(dialog).toContainText('Исходный словарь и готовые файлы останутся на диске.');
  expect(
    await contrast(dialog.getByRole('button', { name: 'Удалить', exact: true })),
  ).toBeGreaterThanOrEqual(4.5);
  await dialog.getByRole('button', { name: 'Отмена', exact: true }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator('.p-accordionheader')).toHaveCount(1);
  await page.getByRole('button', { name: 'Удалить', exact: true }).click();
  await dialog.getByRole('button', { name: 'Удалить', exact: true }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator('.p-accordionheader')).toHaveCount(0);
  const full = await page.locator('.source-composer').evaluate((el) => {
    const main = el.closest('.conversion-workspace')!;
    return (
      el.getBoundingClientRect().height /
      (main.getBoundingClientRect().height - parseFloat(getComputedStyle(main).paddingBottom))
    );
  });
  expect(full).toBeCloseTo(1, 2);
});
