import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
function flatten(value, prefix = '') {
  return Object.fromEntries(
    Object.entries(value).flatMap(([key, item]) => {
      const name = prefix ? `${prefix}.${key}` : key;
      return typeof item === 'object' && item !== null && !Array.isArray(item)
        ? Object.entries(flatten(item, name))
        : [[name, item]];
    }),
  );
}
function slots(text) {
  return [...new Set([...String(text).matchAll(/\{(\w+)\}/g)].map((m) => m[1]))].sort();
}
test('русский и английский каталоги покрывают одинаковые ключи и параметры', async () => {
  for (const base of ['locales', 'website/locales']) {
    const ru = flatten(JSON.parse(await readFile(`${base}/ru.json`, 'utf8')));
    const en = flatten(JSON.parse(await readFile(`${base}/en.json`, 'utf8')));
    assert.deepEqual(Object.keys(ru).sort(), Object.keys(en).sort());
    for (const key of Object.keys(ru)) {
      assert.deepEqual(slots(ru[key]), slots(en[key]), `${base}/${key}`);
      assert.ok(String(en[key]).trim().length > 0, `Empty ${key}`);
    }
  }
});
test('ссылки поддержки раздельны для двух языков', async () => {
  const config = JSON.parse(await readFile('config/locales.json', 'utf8'));
  assert.equal(config.default_locale, 'en');
  assert.equal(config.locales.en.support_url, 'https://hipolink.net/darkbear_ru/tips');
  assert.equal(config.locales.ru.support_url, 'https://pay.cloudtips.ru/p/7e782291');
});

test('все сообщения приложения имеют корректный синтаксис Vue I18n', async () => {
  const { baseCompile } = await import('@intlify/message-compiler');
  for (const lang of ['ru', 'en']) {
    const catalog = flatten(JSON.parse(await readFile(`locales/${lang}.json`, 'utf8')));
    for (const [key, value] of Object.entries(catalog)) {
      if (typeof value !== 'string') continue;
      const errors = [];
      baseCompile(value, { jit: true, onError: (error) => errors.push(error.message) });
      assert.deepEqual(errors, [], `${lang}/${key}`);
    }
  }
});
