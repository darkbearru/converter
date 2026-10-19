import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
import { JSDOM } from 'jsdom';

async function page({ agent, platform = '', touch = 0, hints }) {
  const html = await readFile('website/index.html', 'utf8');
  const dom = new JSDOM(html, { runScripts: 'outside-only' });
  const { navigator } = dom.window;
  for (const [key, value] of Object.entries({
    userAgent: agent,
    platform,
    maxTouchPoints: touch,
    userAgentData: hints,
  }))
    Object.defineProperty(navigator, key, { value, configurable: true });
  const source = await readFile('website/downloads.js', 'utf8');
  dom.window.eval(source);
  return dom;
}

test('macOS и Windows получают один акцентный установщик и закрытый выбор другой системы', async () => {
  for (const [agent, platform, system, expected, alternative] of [
    [
      'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)',
      'MacIntel',
      'macos',
      '{{MAC_URL}}',
      '{{WIN_URL}}',
    ],
    ['Mozilla/5.0 (Windows NT 10.0; Win64; x64)', 'Win32', 'windows', '{{WIN_URL}}', '{{MAC_URL}}'],
  ]) {
    const dom = await page({ agent, platform });
    const document = dom.window.document;
    const primary = document.querySelectorAll('.download-link:not(.download-alternative)');
    assert.equal(primary.length, 1, system);
    assert.equal(primary[0].dataset.downloadSystem, system);
    assert.equal(primary[0].getAttribute('href'), expected);
    const others = document.querySelector('.other-systems');
    assert.equal(others.hidden, false);
    assert.equal(others.open, false);
    assert.equal(others.querySelector('.download-alternative').getAttribute('href'), alternative);
    assert.equal(document.querySelectorAll('.download-link').length, 2);
    dom.window.close();
  }
});

test('телефоны, iPad, Linux и неизвестная ОС получают оба серых установщика', async () => {
  for (const input of [
    { agent: 'Mozilla/5.0 (X11; Linux x86_64)', platform: 'Linux x86_64' },
    { agent: 'Mozilla/5.0 (Linux; Android 15) Mobile', platform: 'Linux aarch64' },
    { agent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)', platform: 'iPhone' },
    { agent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)', platform: 'MacIntel', touch: 5 },
    { agent: '', platform: '' },
  ]) {
    const dom = await page(input);
    const document = dom.window.document;
    assert.equal(
      document.querySelectorAll('.download-link:not(.download-alternative)').length,
      0,
      input.agent,
    );
    assert.equal(document.querySelectorAll('.download-options > .download-alternative').length, 2);
    assert.equal(document.querySelector('.other-systems').hidden, true);
    dom.window.close();
  }
});

test('Client Hints определяют платформу при сокращённом User Agent', async () => {
  const dom = await page({ agent: 'Mozilla/5.0', hints: { platform: 'Windows', mobile: false } });
  assert.equal(
    dom.window.document.querySelector('.download-link:not(.download-alternative)').dataset
      .downloadSystem,
    'windows',
  );
  dom.window.close();
});
