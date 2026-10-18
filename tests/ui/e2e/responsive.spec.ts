import { expect, test } from '@playwright/test';
import { screenUrl } from './helpers';
test('панель занимает свободное место и уступает истории до 70/30', async ({ page }) => {
  const sizes = () =>
    page.evaluate(() => {
      const main = document.querySelector('.conversion-workspace')!;
      const available =
        main.getBoundingClientRect().height - parseFloat(getComputedStyle(main).paddingBottom);
      const composer = document.querySelector('.source-composer')!.getBoundingClientRect();
      const history = document.querySelector('.completed-history')?.getBoundingClientRect();
      return {
        available,
        composer: composer.height,
        history: history?.height ?? 0,
        overflow:
          document.documentElement.scrollHeight > innerHeight ||
          document.documentElement.scrollWidth > innerWidth,
      };
    });
  await page.goto(screenUrl());
  await expect(page.getByRole('button', { name: 'Выбрать словари', exact: true })).toBeVisible();
  const empty = await sizes();
  expect(empty.composer / empty.available).toBeCloseTo(1, 2);
  await page.goto(screenUrl('history-one'));
  await expect(page.locator('.p-accordionheader')).toHaveCount(1);
  const one = await sizes();
  expect(one.composer).toBeLessThan(empty.composer);
  expect(one.composer / one.available).toBeGreaterThan(0.3);
  await page.goto(screenUrl('history-many'));
  await expect(page.locator('.p-accordionheader')).toHaveCount(12);
  const many = await sizes();
  expect(many.composer / many.available).toBeCloseTo(0.3, 2);
  expect(many.history / many.available).toBeCloseTo(0.7, 2);
  await page.locator('.p-accordionheader').first().click();
  await expect(page.locator('.result-path').first()).toBeVisible();
  expect((await sizes()).composer / many.available).toBeCloseTo(0.3, 2);
  await page.setViewportSize({ width: 540, height: 400 });
  const small = await sizes();
  expect(small.composer / small.available).toBeCloseTo(0.3, 2);
  expect(small.overflow).toBe(false);
  await page.getByRole('button', { name: 'Выбрать словари', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Конвертировать', exact: true })).toBeInViewport();
  await page.getByRole('button', { name: 'Убрать выбранный источник', exact: true }).click();
  await page.getByRole('button', { name: 'Добавить ссылку', exact: true }).click();
  await page.getByLabel('Ссылка на словарь', { exact: true }).fill('https://example.org/main.dsl');
  await page.getByRole('button', { name: 'Скачать', exact: true }).click();
  await expect(page.getByRole('progressbar', { name: 'Скачивание', exact: true })).toBeInViewport({
    ratio: 1,
  });
  await expect(
    page.getByRole('button', { name: 'Отменить скачивание', exact: true }),
  ).toBeInViewport({ ratio: 1 });
  await page.goto(screenUrl('history-active'));
  await expect(page.locator('.active-task')).toHaveCount(2);
  const active = await sizes();
  expect(active.composer / active.available).toBeGreaterThanOrEqual(0.299);
  expect(active.overflow).toBe(false);
});
test('длинное имя, журнал и системная тема работают в изменяемом окне', async ({ page }) => {
  await page.goto(screenUrl('long'));
  const before = await page.locator('body').evaluate((el) => getComputedStyle(el).backgroundColor);
  const current = await page.evaluate(() => matchMedia('(prefers-color-scheme: dark)').matches);
  await page.emulateMedia({ colorScheme: current ? 'light' : 'dark' });
  await expect
    .poll(() => page.locator('body').evaluate((el) => getComputedStyle(el).backgroundColor))
    .not.toBe(before);
  await page.setViewportSize({ width: 540, height: 400 });
  await page.locator('.p-accordionheader').click();
  await page.getByRole('button', { name: /Журнал · 70/ }).click();
  const log = page.locator('.diagnostic-list');
  await expect(log.locator('li')).toHaveCount(50);
  await expect(log.locator('img')).toHaveCount(0);
  await page.getByRole('button', { name: 'Далее', exact: true }).click();
  await expect(log.locator('li')).toHaveCount(20);
  await expect
    .poll(() => page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
    .toBe(true);
  await expect(page.getByRole('button', { name: 'Выбрать словари', exact: true })).toBeVisible();
});
test('новые задачи образуют стопку, отмена и reduced motion сохраняют действия', async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto(screenUrl('stack'));
  await expect(page.locator('.active-task')).toHaveCount(2);
  for (let i = 0; i < 3; i++) {
    await page.getByRole('button', { name: 'Выбрать словари', exact: true }).click();
    await page.getByRole('button', { name: 'Конвертировать', exact: true }).click();
  }
  await expect(page.locator('.active-task')).toHaveCount(5);
  await expect
    .poll(() =>
      page
        .locator('.task-stack')
        .evaluate((el) => el.scrollHeight - el.clientHeight - el.scrollTop),
    )
    .toBeLessThan(2);
  await expect
    .poll(() =>
      page
        .locator('.active-task')
        .last()
        .evaluate((el) => getComputedStyle(el).transform),
    )
    .toBe('none');
  await page
    .locator('.active-task')
    .last()
    .getByRole('button', { name: 'Отменить main.dsl', exact: true })
    .click();
  await expect(page.locator('.active-task')).toHaveCount(4);
  await page.locator('.p-accordionheader').click();
  await expect(page.getByRole('button', { name: 'Запустить заново', exact: true })).toBeVisible();
});
