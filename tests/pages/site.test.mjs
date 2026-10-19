import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { test } from 'node:test';

const script = resolve('scripts/pages/build.mjs');
const repository = 'https://github.com/darkbearru/converter';
function release(version = '0.2.4') {
  return {
    tag_name: `v${version}`,
    draft: false,
    prerelease: false,
    assets: [
      `Converter_${version}_aarch64.dmg`,
      `Converter_${version}_x64-setup.exe`,
      'SHA256SUMS',
    ].map((name) => ({
      name,
      state: 'uploaded',
      size: name.endsWith('.exe') ? 222137224 : 7592986,
      browser_download_url: `${repository}/releases/download/v${version}/${name}`,
    })),
  };
}
async function context(t) {
  const root = await mkdtemp(join(tmpdir(), 'converter-pages-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  return { root, output: join(root, 'site') };
}
function run(output, data, base) {
  return spawnSync(process.execPath, [script, '--output', output], {
    input: JSON.stringify(data),
    encoding: 'utf8',
    env: { ...process.env, ...(base ? { PAGES_BASE_URL: base } : {}) },
  });
}

test('страница сразу содержит два прямых установщика и новую версию без JavaScript и GitHub API у посетителя', async (t) => {
  const { output } = await context(t);
  for (const version of ['0.2.4', '0.2.10']) {
    const result = run(output, release(version));
    assert.equal(result.status, 0, result.stderr);
    const html = await readFile(join(output, 'index.html'), 'utf8');
    assert.ok(html.includes(`Version ${version}`));
    assert.ok(
      html.includes(`${repository}/releases/download/v${version}/Converter_${version}_aarch64.dmg`),
    );
    assert.ok(
      html.includes(
        `${repository}/releases/download/v${version}/Converter_${version}_x64-setup.exe`,
      ),
    );
    assert.equal((html.match(/class="download-link download-alternative"/g) ?? []).length, 2);
    assert.ok(!html.includes('SHA256SUMS'));
    assert.ok(!html.includes('{{'));
    assert.ok(!html.includes('api.github.com'));
    assert.ok(html.includes('language.js'));
    assert.ok(html.includes('downloads.js'));
    await readFile(join(output, 'downloads.js'));
    await readFile(join(output, 'book.svg'));
    await readFile(join(output, 'style.css'));
  }
});

test('предпросмотр остаётся внутри своего адреса и сохраняет опубликованную главную', async (t) => {
  const { output } = await context(t);
  await mkdir(output, { recursive: true });
  await writeFile(join(output, 'index.html'), 'Published home');
  const preview = join(output, 'preview');
  const result = run(preview, release(), 'https://darkbearru.github.io/converter/preview/');
  assert.equal(result.status, 0, result.stderr);
  assert.equal(await readFile(join(output, 'index.html'), 'utf8'), 'Published home');
  for (const language of ['ru', 'en']) {
    const html = await readFile(join(preview, language, 'index.html'), 'utf8');
    assert.ok(html.includes(`href="https://darkbearru.github.io/converter/preview/${language}/"`));
    assert.ok(html.includes('content="https://darkbearru.github.io/converter/preview/interface'));
    assert.ok(!html.includes('href="https://darkbearru.github.io/converter/ru/"'));
    for (const asset of [...html.matchAll(/(?:src|href)="([^"]+)"/g)]
      .map((match) => match[1])
      .filter((value) => !value.startsWith('http') && !value.startsWith('#'))) {
      await readFile(
        resolve(preview, language, asset.endsWith('/') ? asset + 'index.html' : asset),
      );
    }
  }
});

test('неправильный базовый адрес не затирает существующую сборку', async (t) => {
  const { output } = await context(t);
  assert.equal(run(output, release()).status, 0);
  const original = await readFile(join(output, 'index.html'), 'utf8');
  for (const base of [
    'http://example.com/preview/',
    'https://example.com/preview',
    'https://example.com/preview/?query=1',
    'https://example.com/preview/#anchor',
    'https://user:password@example.com/preview/',
  ]) {
    assert.notEqual(run(output, release(), base).status, 0, base);
    assert.equal(await readFile(join(output, 'index.html'), 'utf8'), original);
  }
});

test('все локальные изображения и стили страницы доступны в публичной сборке', async (t) => {
  const { output } = await context(t);
  const result = run(output, release());
  assert.equal(result.status, 0, result.stderr);
  const html = await readFile(join(output, 'index.html'), 'utf8');
  const localAssets = [...html.matchAll(/(?:src|href)="([^"]+)"/g)]
    .map((match) => match[1])
    .filter((url) => !url.startsWith('https:') && !url.startsWith('#') && url !== './');
  for (const asset of new Set(localAssets)) {
    const content = await readFile(
      join(output, asset.endsWith('/') ? asset + 'index.html' : asset),
    );
    assert.ok(content.length > 0, `Пустой ресурс: ${asset}`);
  }
});

test('неполный, предварительный или подменённый релиз не затирает рабочую страницу', async (t) => {
  const { output } = await context(t);
  assert.equal(run(output, release()).status, 0);
  const original = await readFile(join(output, 'index.html'), 'utf8');
  const invalid = [
    { ...release(), draft: true },
    { ...release(), prerelease: true },
    { ...release(), tag_name: '<script>' },
    { ...release(), assets: release().assets.slice(1) },
    {
      ...release(),
      assets: release().assets.map((asset) => ({
        ...asset,
        browser_download_url: 'https://example.com/foreign.exe',
      })),
    },
    { ...release(), assets: release().assets.map((asset) => ({ ...asset, state: 'new' })) },
  ];
  for (const data of invalid) {
    assert.notEqual(run(output, data).status, 0);
    assert.equal(await readFile(join(output, 'index.html'), 'utf8'), original);
  }
});

test('оба языка имеют одинаковые установщики, свои метаданные и отдельную поддержку', async (t) => {
  const { output } = await context(t);
  const result = run(output, release('0.2.6'));
  assert.equal(result.status, 0, result.stderr);
  for (const lang of ['ru', 'en']) {
    const html = await readFile(join(output, lang, 'index.html'), 'utf8');
    assert.ok(html.includes(`lang="${lang}"`));
    assert.ok(html.includes(`/converter/${lang}/`));
    assert.ok(html.includes('hreflang="ru"') && html.includes('hreflang="en"'));
    assert.ok(
      html.includes('Converter_0.2.6_aarch64.dmg') &&
        html.includes('Converter_0.2.6_x64-setup.exe'),
    );
    assert.equal(html.includes('pay.cloudtips.ru'), lang === 'ru');
    assert.equal(html.includes('https://hipolink.net/darkbear_ru/tips'), lang === 'en');
    assert.ok(!html.includes('{{'));
    for (const asset of [...html.matchAll(/(?:src|href)="([^"]+)"/g)]
      .map((m) => m[1])
      .filter((value) => !value.startsWith('http') && !value.startsWith('#'))) {
      await readFile(resolve(output, lang, asset.endsWith('/') ? asset + 'index.html' : asset));
    }
  }
});
