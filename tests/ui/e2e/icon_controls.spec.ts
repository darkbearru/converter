import { expect, test } from '@playwright/test';
import { screenUrl } from './helpers';
import type { Locator } from '@playwright/test';

function frames(button: Locator) {
  return button.evaluate(async (el) => {
    const result = [];
    for (let frame = 0; frame < 24; frame++) {
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
      const style = getComputedStyle(el);
      result.push({
        border: style.borderTopWidth,
        background: style.backgroundColor,
        shadow: style.boxShadow,
        width: el.getBoundingClientRect().width,
      });
    }
    return result;
  });
}

test('кнопки ссылки и настроек не рисуют рамку во время наведения и ухода курсора', async ({
  page,
  browserName,
}) => {
  await page.goto(screenUrl());
  for (const name of ['Добавить ссылку', 'Настройки конвертации']) {
    const button = page.getByRole('button', { name, exact: true });
    const entering = frames(button);
    await button.hover();
    const entered = await entering;
    const leaving = frames(button);
    await page.mouse.move(0, 0);
    const left = await leaving;
    for (const samples of [entered, left]) {
      expect(
        samples.filter(
          (sample) =>
            sample.border !== '0px' ||
            sample.shadow !== 'none' ||
            !['transparent', 'rgba(0, 0, 0, 0)'].includes(sample.background) ||
            sample.width !== 24,
        ),
      ).toEqual([]);
    }
  }
  await page.keyboard.press(
    browserName === 'webkit' && process.platform === 'darwin' ? 'Alt+Tab' : 'Tab',
  );
  const link = page.getByRole('button', { name: 'Добавить ссылку', exact: true });
  await expect(link).toBeFocused();
  await expect
    .poll(() => link.evaluate((el) => getComputedStyle(el).outlineColor))
    .not.toBe('rgba(0, 0, 0, 0)');
  const focus = await link.evaluate((el) => {
    const style = getComputedStyle(el);
    return {
      visible: el.matches(':focus-visible'),
      width: parseFloat(style.outlineWidth),
      color: style.outlineColor,
    };
  });
  expect(focus.visible).toBe(true);
  expect(focus.width).toBeGreaterThan(0);
  expect(focus.color).not.toBe('rgba(0, 0, 0, 0)');
});

test('крестик предупреждений компактный и закрывает журнал с возвратом фокуса', async ({
  page,
}) => {
  await page.goto(screenUrl('warning'));
  await page.locator('.p-accordionheader').click();
  const link = page.getByRole('link', { name: /ещё 28\s172 предупреждения/ });
  await link.click();
  const dialog = page.getByRole('dialog', { name: 'Предупреждения', exact: true });
  await expect(dialog).not.toHaveClass(/p-dialog-enter/);
  const close = dialog.locator('.p-dialog-close-button');
  const sizes = await close.evaluate((el) => {
    const button = el.getBoundingClientRect();
    const icon = el.querySelector('.app-icon')!.getBoundingClientRect();
    return {
      button: Math.max(button.width, button.height),
      icon: Math.max(icon.width, icon.height),
    };
  });
  expect(sizes.button).toBeLessThanOrEqual(24);
  expect(sizes.icon).toBeLessThanOrEqual(16);
  await close.click();
  await expect(dialog).toBeHidden();
  await expect(link).toBeFocused();
});
