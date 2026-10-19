import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
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
function run(output, data) {
  return spawnSync(process.execPath, [script, '--output', output], {
    input: JSON.stringify(data),
    encoding: 'utf8',
  });
}

test('страница сразу содержит два прямых установщика и новую версию без JavaScript и GitHub API у посетителя', async (t) => {
  const { output } = await context(t);
  for (const version of ['0.2.4', '0.2.10']) {
    const result = run(output, release(version));
    assert.equal(result.status, 0, result.stderr);
    const html = await readFile(join(output, 'index.html'), 'utf8');
    assert.ok(html.includes(`Версия ${version}`));
    assert.ok(
      html.includes(`${repository}/releases/download/v${version}/Converter_${version}_aarch64.dmg`),
    );
    assert.ok(
      html.includes(
        `${repository}/releases/download/v${version}/Converter_${version}_x64-setup.exe`,
      ),
    );
    assert.equal((html.match(/class="download-link"/g) ?? []).length, 2);
    assert.ok(!html.includes('SHA256SUMS'));
    assert.ok(!html.includes('{{'));
    assert.ok(!html.includes('<script'));
    await readFile(join(output, 'book.svg'));
    await readFile(join(output, 'style.css'));
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
