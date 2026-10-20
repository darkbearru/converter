import { expect, test } from '@playwright/test';
import { screenUrl } from './helpers';

for (const language of ['ru', 'en']) {
  test(`включённые части: одно задание и уведомление ${language}`, async ({ page }, testInfo) => {
    const ru = language === 'ru';
    await page.goto(screenUrl('included-parts') + `&lang=${language}`);
    await page
      .getByRole('button', { name: ru ? 'Выбрать словари' : 'Choose dictionaries', exact: true })
      .click();
    await page
      .getByRole('button', { name: ru ? 'Конвертировать' : 'Convert', exact: true })
      .click();
    const dialog = page.getByRole('dialog', {
      name: ru ? 'Словари уже включены' : 'Dictionaries already included',
    });
    await expect(dialog).toBeVisible();
    await expect(dialog).toContainText('大БКРС - 260610');
    await expect(dialog).toContainText('dabkrs_2.dsl');
    await expect(dialog).toContainText('dabkrs_3.dsl');
    await expect(page.locator('.active-task')).toHaveCount(1);
    await expect(page.locator('.active-task')).toContainText('dabkrs_1.dsl');
    expect(await dialog.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);
    await page.screenshot({ path: testInfo.outputPath(`included-${language}.png`) });
    await dialog
      .locator('.p-dialog-footer')
      .getByRole('button', { name: ru ? 'Закрыть' : 'Close', exact: true })
      .click();
    await expect(dialog).toHaveCount(0);
    await expect(
      page.getByRole('button', {
        name: ru ? 'Выбрать словари' : 'Choose dictionaries',
        exact: true,
      }),
    ).toBeVisible();
    await expect(page.locator('.active-task')).toHaveCount(1);
  });
}
