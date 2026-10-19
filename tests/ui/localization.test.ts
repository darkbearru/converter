import { formatBytes, formatQuantity } from '../../src/shared/format';
import { nextTick } from 'vue';
import { syncPrimeVue } from '../../src/shared/i18n/primevue';
import { formatMessage } from '../../src/shared/i18n/messages';
import { describe, expect, it, vi } from 'vitest';
import { createLocalization, resolveLocale } from '../../src/shared/i18n';
import { createHarness } from './harness/bridge';

describe('выбор языка', () => {
  it.each([
    ['ru-RU', 'ru'],
    ['RU_ru', 'ru'],
    ['en-US', 'en'],
    ['de-DE', 'en'],
    [null, 'en'],
  ])('использует только основной язык %s → %s', (system, expected) => {
    expect(resolveLocale('system', system)).toBe(expected);
  });
  it('ручной выбор имеет приоритет над системой', () => {
    expect(resolveLocale('ru', 'en-US')).toBe('ru');
    expect(resolveLocale('en', 'ru-RU')).toBe('en');
  });
  it('применяет только успешно сохранённый выбор', async () => {
    const { bridge } = createHarness('local');
    const localization = createLocalization(
      { language: 'ru', locale: 'ru', support_url: null },
      bridge,
    );
    vi.spyOn(bridge, 'setAppLanguage').mockRejectedValueOnce(new Error('readonly'));
    await expect(localization.setLanguage('en')).rejects.toThrow('readonly');
    expect(localization.preferences.locale).toBe('ru');
    await localization.setLanguage('en');
    expect(localization.preferences.locale).toBe('en');
    expect(localization.i18n.global.t('app.close')).toBe('Close');
    expect(localization.preferences.support_url).toBeNull();
  });
});

it('переводит параметры буквально и читает старые сообщения', () => {
  createLocalization(
    { language: 'en', locale: 'en', support_url: null },
    createHarness('local').bridge,
  );
  expect(
    formatMessage({
      message: 'fallback',
      message_key: 'messages.missing_resource',
      message_args: { raw: '<b>{name}</b>' },
    }),
  ).toBe('Missing resource: <b>{name}</b>');
  expect(formatMessage({ message: 'старый журнал' })).toBe('старый журнал');
  expect(formatMessage({ message: 'unknown fallback', message_key: 'messages.future_key' })).toBe(
    'unknown fallback',
  );
});

it('форматирует количества и размеры по выбранному языку', () => {
  const { bridge } = createHarness('local');
  createLocalization({ language: 'ru', locale: 'ru', support_url: null }, bridge);
  expect([1, 2, 5, 11, 21].map((n) => formatQuantity(n, 'articles'))).toEqual([
    '1 статья',
    '2 статьи',
    '5 статей',
    '11 статей',
    '21 статья',
  ]);
  expect(formatBytes(1024)).toBe('1 Кб');
  createLocalization({ language: 'en', locale: 'en', support_url: null }, bridge);
  expect(formatQuantity(1, 'articles')).toBe('1 entry');
  expect(formatQuantity(2, 'articles')).toBe('2 entries');
  expect(formatBytes(1024)).toBe('1 KB');
});

it('сохраняет defaults PrimeVue и переводит объявления выбора', async () => {
  const localization = createLocalization(
    { language: 'ru', locale: 'ru', support_url: null },
    createHarness('local').bridge,
  );
  const target = {
    locale: {
      startsWith: 'Starts with',
      selectionMessage: 'default',
      emptySelectionMessage: 'default',
      aria: { filter: 'Filter', close: 'Close' },
    },
  };
  const stop = syncPrimeVue(localization, target);
  expect(target.locale.startsWith).toBe('Starts with');
  expect(target.locale.aria.filter).toBe('Filter');
  expect(target.locale.selectionMessage).toBe('Выбран {0} элемент');
  expect(target.locale.emptySelectionMessage).toBe('Ничего не выбрано');
  await localization.setLanguage('en');
  await nextTick();
  expect(target.locale.selectionMessage).toBe('{0} item selected');
  expect(target.locale.emptySelectionMessage).toBe('No item selected');
  stop();
});
