import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { test } from 'node:test';

const script = resolve('scripts/build-version.mjs');
const names = ['converter-core', 'converter-desktop'];
async function fixture(t, version = '0.2.9') {
  const root = await mkdtemp(join(tmpdir(), 'converter-version-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  await mkdir(join(root, 'src-tauri'));
  const files = {
    'package.json': JSON.stringify({ name: 'converter', version }),
    'package-lock.json': JSON.stringify({
      name: 'converter',
      version,
      packages: { '': { version }, 'node_modules/example': { version: '1.2.3' } },
    }),
    'src-tauri/tauri.conf.json': JSON.stringify({ version }),
    'Cargo.toml': `[workspace.package]\nversion = "${version}"\nedition = "2024"\n`,
    'Cargo.lock':
      'version = 4\n\n' +
      names.map((name) => `[[package]]\nname = "${name}"\nversion = "${version}"\n`).join('\n') +
      '\n[[package]]\nname = "serde"\nversion = "1.0.0"\nsource = "registry+example"\n',
  };
  for (const [name, contents] of Object.entries(files)) await writeFile(join(root, name), contents);
  return { root, files };
}
function run(root, ...args) {
  return spawnSync(process.execPath, [script, ...args], { cwd: root, encoding: 'utf8' });
}
async function json(root, path) {
  return JSON.parse(await readFile(join(root, path), 'utf8'));
}

test('новая сборка повышает patch и синхронизирует npm, Tauri и Rust без смены зависимостей', async (t) => {
  const { root } = await fixture(t);
  const result = run(root);
  assert.equal(result.status, 0, result.stderr);
  assert.equal((await json(root, 'package.json')).version, '0.2.10');
  assert.equal((await json(root, 'src-tauri/tauri.conf.json')).version, '0.2.10');
  const lock = await json(root, 'package-lock.json');
  assert.equal(lock.version, '0.2.10');
  assert.equal(lock.packages[''].version, '0.2.10');
  assert.equal(lock.packages['node_modules/example'].version, '1.2.3');
  assert.match(await readFile(join(root, 'Cargo.toml'), 'utf8'), /version = "0\.2\.10"/);
  const cargo = await readFile(join(root, 'Cargo.lock'), 'utf8');
  for (const name of names) assert.ok(cargo.includes(`name = "${name}"\nversion = "0.2.10"`));
  assert.ok(cargo.includes('name = "serde"\nversion = "1.0.0"'));
  assert.equal(run(root).status, 0);
  assert.equal((await json(root, 'package.json')).version, '0.2.11');
});

test('другая платформа и повторная CI-сборка используют подготовленную версию', async (t) => {
  const { root, files } = await fixture(t, '1.4.12');
  const result = run(root, '--reuse');
  assert.equal(result.status, 0, result.stderr);
  for (const [name, contents] of Object.entries(files))
    assert.equal(await readFile(join(root, name), 'utf8'), contents);
});

test('расхождение версий останавливает сборку до изменения файлов', async (t) => {
  const { root, files } = await fixture(t);
  files['src-tauri/tauri.conf.json'] = '{"version":"0.1.0"}';
  await writeFile(join(root, 'src-tauri/tauri.conf.json'), files['src-tauri/tauri.conf.json']);
  const result = run(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /версии/i);
  for (const [name, contents] of Object.entries(files))
    assert.equal(await readFile(join(root, name), 'utf8'), contents);
});

test('нестандартная версия не заменяется молча', async (t) => {
  const { root, files } = await fixture(t, '0.3.0-beta.1');
  const result = run(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /верси/i);
  assert.equal(await readFile(join(root, 'package.json'), 'utf8'), files['package.json']);
});
