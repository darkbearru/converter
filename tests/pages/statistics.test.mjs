import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
import { JSDOM } from 'jsdom';

async function page(t, locale, response) {
  const html = await readFile('website/index.html', 'utf8');
  const dom = new JSDOM(html, { runScripts: 'outside-only' });
  t.after(() => dom.window.close());
  dom.window.document.documentElement.lang = locale;
  const counter = dom.window.document.querySelector('[data-download-statistics]');
  assert.ok(counter, 'Счётчик присутствует в шаблоне');
  counter.hidden = true;
  dom.window.fetch = response;
  dom.window.AbortSignal = AbortSignal;
  dom.window.eval(await readFile('website/statistics.js', 'utf8'));
  await new Promise((accept) => setImmediate(accept));
  return counter;
}

test('число скачиваний форматируется по языку страницы, включая настоящий ноль', async (t) => {
  for (const [locale, total, expected] of [
    ['ru', 12345, '12\u00a0345'],
    ['en', 12345, '12,345'],
    ['en', 0, '0'],
  ]) {
    const counter = await page(t, locale, async () => ({
      ok: true,
      json: async () => ({ total }),
    }));
    assert.equal(counter.hidden, false);
    assert.equal(counter.querySelector('[data-download-total]').textContent, expected);
  }
});

test('сбой или неправильные данные сохраняют последнее число и не подменяют его нулём', async (t) => {
  for (const response of [
    async () => {
      throw new Error('Offline');
    },
    async () => ({ ok: false }),
    ...[-1, null, '50', 0.5].map((total) => async () => ({
      ok: true,
      json: async () => ({ total }),
    })),
  ]) {
    const counter = await page(t, 'en', response);
    assert.equal(counter.hidden, true);
    counter.hidden = false;
    counter.querySelector('[data-download-total]').textContent = '42';
    counter.ownerDocument.defaultView.eval(await readFile('website/statistics.js', 'utf8'));
    await new Promise((accept) => setImmediate(accept));
    assert.equal(counter.hidden, false);
    assert.equal(counter.querySelector('[data-download-total]').textContent, '42');
  }
});
