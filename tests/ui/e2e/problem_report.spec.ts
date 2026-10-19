import { expect, test } from '@playwright/test';
import { screenUrl } from './helpers';

test('обе кнопки сообщения компактны, доступны и остаются в строке при минимальном окне', async ({
  page,
  browserName,
}, testInfo) => {
  await page.goto(screenUrl('succeeded'));
  const header = page.getByRole('banner');
  const general = header.getByRole('button', { name: 'Сообщить о проблеме' });
  await expect(general).toBeVisible();
  await page.getByRole('button', { name: 'main.dsl' }).click();
  const actions = page.getByRole('group', { name: 'Действия с конвертацией' });
  const report = actions.getByRole('button', { name: 'Сообщить о проблеме' });
  for (const button of [general]) {
    await expect(button).toHaveAttribute('title', 'Сообщить о проблеме');
    expect(
      await button.evaluate((element) => {
        const box = element.getBoundingClientRect();
        const icon = element.querySelector('.app-icon')!.getBoundingClientRect();
        return {
          width: box.width,
          height: box.height,
          iconWidth: icon.width,
          iconHeight: icon.height,
        };
      }),
    ).toEqual({ width: 24, height: 24, iconWidth: 16, iconHeight: 16 });
    await button.hover();
    await expect(button).toHaveCSS('border-top-width', '0px');
    await expect(button).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)');
    await expect(button).toHaveCSS('box-shadow', 'none');
    await button.click();
    await expect(page.getByRole('alert')).toHaveCount(0);
  }
  await expect(report).toHaveClass(/p-button-outlined/);
  await expect(report).toHaveClass(/p-button-secondary/);
  await expect(report.locator('.app-icon')).toHaveCSS('width', '18px');
  await expect(report.locator('.app-icon')).toHaveCSS('height', '18px');
  const heights = await actions
    .getByRole('button')
    .evaluateAll((buttons) => buttons.map((button) => button.getBoundingClientRect().height));
  expect(Math.max(...heights) - Math.min(...heights)).toBeLessThan(1);
  await page.screenshot({ path: testInfo.outputPath('problem-report.png') });
  await page.setViewportSize({ width: 540, height: 400 });
  await expect(report).toBeInViewport();
  const tops = await actions.getByRole('button').evaluateAll((buttons) =>
    buttons.map((button) => {
      const box = button.getBoundingClientRect();
      return box.y + box.height / 2;
    }),
  );
  expect(Math.max(...tops) - Math.min(...tops)).toBeLessThan(1);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(
    true,
  );
  await actions.getByRole('button', { name: 'Удалить', exact: true }).focus();
  await page.keyboard.press(
    browserName === 'webkit' && process.platform === 'darwin' ? 'Alt+Tab' : 'Tab',
  );
  await expect(report).toBeFocused();
  await expect(report).toHaveCSS('outline-style', 'solid');
  await page.screenshot({ path: testInfo.outputPath('problem-report-narrow.png') });
});
