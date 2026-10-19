import { expect, test } from '@playwright/test';
import { screenUrl, step } from './helpers';

test('обновление предлагается при запуске, окно о программе доступно и при минимальном размере', async ({
  page,
}, testInfo) => {
  await page.goto(screenUrl('update-available'));
  const notice = page.getByRole('dialog', { name: 'Доступно обновление' });
  await expect(notice).toBeVisible();
  await expect(notice).toContainText('0.3.0');
  await expect(notice.locator('.update-content')).toBeFocused();
  await expect(notice.getByRole('button', { name: 'Закрыть', exact: true })).not.toBeFocused();
  await page.screenshot({ path: testInfo.outputPath('update-notification.png') });
  await notice.getByRole('button', { name: 'Позже' }).click();
  await step(page, 'about');
  const about = page.getByRole('dialog', { name: 'О программе' });
  await expect(about).toBeVisible();
  await expect(about).toContainText('© Alexey Abramenko');
  await expect(about.locator('.about-content')).toBeFocused();
  await expect(
    about.getByRole('button', { name: 'Закрыть', exact: true }).first(),
  ).not.toBeFocused();
  const thanks = about.getByRole('link', { name: 'Сказать спасибо' });
  await expect(thanks).toHaveAttribute('href', 'https://pay.cloudtips.ru/p/7e782291');
  const centers = await thanks.evaluate((element) =>
    [...element.children].map((child) => {
      const box = child.getBoundingClientRect();
      return box.y + box.height / 2;
    }),
  );
  expect(Math.abs(centers[0]! - centers[1]!)).toBeLessThan(1);
  await expect(thanks.locator('.app-icon')).toHaveCSS('color', 'rgb(249, 97, 51)');
  await expect(about.getByRole('button', { name: 'Проверить обновления' })).toHaveCount(0);
  await expect(about.locator('.about-identity')).toContainText(
    'Версия 0.2.3 (доступна версия 0.3.0)',
  );
  await expect(notice).not.toBeVisible();
  await page.screenshot({ path: testInfo.outputPath('about-updates.png') });
  await page.setViewportSize({ width: 540, height: 400 });
  for (const name of ['Скачать', 'Закрыть']) {
    await expect(about.getByRole('button', { name, exact: true }).last()).toBeInViewport();
  }
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('about-narrow.png') });
  await about.getByRole('button', { name: 'Закрыть', exact: true }).last().click();
  await expect(notice).not.toBeVisible();
});

test('недоступность GitHub не мешает запуску и отображается в окне о программе', async ({
  page,
}) => {
  await page.goto(screenUrl('update-offline'));
  await expect(page.getByRole('button', { name: 'Выбрать словари' })).toBeVisible();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await step(page, 'about');
  const about = page.getByRole('dialog', { name: 'О программе' });
  await expect(about).toContainText('Версия 0.2.3');
  await expect(about.locator('.about-identity')).toContainText(
    'Версия 0.2.3 (не удалось проверить обновления)',
  );
  await expect(about.getByRole('button', { name: 'Скачать' })).toHaveCount(0);
  await expect(about.getByRole('button', { name: 'Проверить обновления' })).toBeEnabled();
});
