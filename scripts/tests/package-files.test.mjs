import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

test('новая сборка включает только текущие пакеты и очищает старые копии из папки выдачи', async (t) => {
  const root = await mkdtemp(join(tmpdir(), 'converter-packages-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const bundle = join(root, 'bundle');
  const destination = join(root, 'output');
  await mkdir(join(bundle, 'dmg'), { recursive: true });
  await mkdir(destination);
  const current = 'Converter_0.2.1_aarch64.dmg';
  const old = 'Converter_0.2.0_aarch64.dmg';
  for (const name of [current, old]) await writeFile(join(bundle, 'dmg', name), name);
  await writeFile(join(destination, old), old);
  await writeFile(join(destination, 'notes.txt'), 'Сохранить');
  const module = await import('../package-files.mjs');
  const result = await module.collectPackageFiles(
    bundle,
    destination,
    'aarch64-apple-darwin',
    '0.2.1',
  );
  assert.deepEqual(result, [join(destination, current)]);
  assert.equal(await readFile(join(destination, current), 'utf8'), current);
  assert.deepEqual((await readdir(destination)).sort(), [current, 'notes.txt'].sort());
});
