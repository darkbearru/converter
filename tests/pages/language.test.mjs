import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { runInNewContext } from 'node:vm';
import { test } from 'node:test';
async function choose(primary, stored, root = true, blocked = false) {
  const source = await readFile('website/language.js', 'utf8');
  let destination = null;
  const links = new Map();
  runInNewContext(source, {
    document: {
      baseURI: 'https://darkbearru.github.io/converter/',
      documentElement: {
        dataset: { languageRoot: String(root), supportedLocales: 'ru,en', defaultLocale: 'en' },
      },
      querySelectorAll: () =>
        ['ru', 'en'].map((language) => ({
          dataset: { siteLanguage: language },
          addEventListener: (_, callback) => links.set(language, callback),
        })),
    },
    navigator: { language: primary, languages: [primary, 'ru-RU'] },
    localStorage: {
      getItem: () => {
        if (blocked) throw Error('denied');
        return stored;
      },
      setItem: () => {
        if (blocked) throw Error('denied');
      },
    },
    location: { replace: (value) => (destination = value), search: '', hash: '' },
    URL,
  });
  return { destination, links };
}
test('корень использует основной язык, ручной выбор и английский fallback', async () => {
  for (const [primary, stored, expected] of [
    ['ru-RU', null, 'ru'],
    ['en-US', null, 'en'],
    ['de-DE', null, 'en'],
    [undefined, null, 'en'],
    ['ru-RU', 'en', 'en'],
    ['de-DE', 'xx', 'en'],
  ]) {
    assert.equal(
      (await choose(primary, stored)).destination,
      `https://darkbearru.github.io/converter/${expected}/`,
    );
  }
});
test('явный язык и отказ localStorage не ломают переходы', async () => {
  assert.equal((await choose('en-US', 'en', false)).destination, null);
  const { destination, links } = await choose('ru-RU', null, true, true);
  assert.equal(destination, 'https://darkbearru.github.io/converter/ru/');
  assert.doesNotThrow(() => links.get('en')());
});
