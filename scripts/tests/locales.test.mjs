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
test('английская поддержка выключена до настройки сервиса', async () => {
  const config = JSON.parse(await readFile('config/locales.json', 'utf8'));
  assert.equal(config.default_locale, 'en');
  assert.equal(config.locales.en.support_url, null);
  assert.equal(config.locales.ru.support_url, 'https://pay.cloudtips.ru/p/7e782291');
});
