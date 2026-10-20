import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { test } from 'node:test';

function run(releases) {
  return spawnSync(process.execPath, ['scripts/collect-download-statistics.mjs'], {
    input: JSON.stringify(releases),
    encoding: 'utf8',
  });
}
const asset = (name, download_count) => ({ name, download_count, state: 'uploaded' });

test('суммируются только DMG и EXE всех опубликованных версий на всех страницах API', () => {
  const result = run([
    [
      {
        id: 3,
        draft: false,
        prerelease: false,
        assets: [
          asset('Converter_0.3.3_aarch64.dmg', 5),
          asset('Converter_0.3.3_x64-setup.exe', 7),
          asset('Converter_aarch64-apple-darwin.app.zip', 500),
          asset('SHA256SUMS', 999),
        ],
      },
      { id: 4, draft: true, assets: [asset('Converter_0.4.0_aarch64.dmg', 1000)] },
    ],
    [
      {
        id: 2,
        draft: false,
        prerelease: false,
        assets: [
          asset('Converter_0.3.0_aarch64.dmg', 11),
          asset('Converter_0.3.0_x64-setup.exe', 13),
          asset('Converter_0.3.0_x64.dmg', 17),
        ],
      },
      {
        id: 1,
        draft: false,
        prerelease: true,
        assets: [asset('Converter_0.1.0-beta.1_x64-setup.exe', 19)],
      },
    ],
  ]);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout), { total: 72 });
});

test('повтор релиза между страницами API не удваивает скачивания', () => {
  const release = { id: 1, draft: false, assets: [asset('Converter_0.3.3_x64-setup.exe', 9)] };
  const result = run([[release], [release]]);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout), { total: 9 });
  assert.deepEqual(JSON.parse(run([]).stdout), { total: 0 });
});

test('некорректные счётчики и ответы API не публикуются как ноль или частичная сумма', () => {
  for (const data of [
    { message: 'API rate limit exceeded' },
    [{ id: 1, draft: false, assets: null }],
    ...[-1, 1.5, '12', null, Number.MAX_SAFE_INTEGER + 1].map((count) => [
      { id: 1, draft: false, assets: [asset('Converter_0.3.3_aarch64.dmg', count)] },
    ]),
    [
      {
        id: 1,
        draft: false,
        assets: [
          asset('Converter_0.3.3_aarch64.dmg', Number.MAX_SAFE_INTEGER),
          asset('Converter_0.3.3_x64-setup.exe', 1),
        ],
      },
    ],
  ]) {
    const result = run(data);
    assert.notEqual(result.status, 0);
    assert.equal(result.stdout, '');
  }
});
