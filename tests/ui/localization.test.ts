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
