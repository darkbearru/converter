import { spawnSync } from 'node:child_process';
import { copyFile, mkdir, readdir, rm } from 'node:fs/promises';
import { join } from 'node:path';

export async function collectPackageFiles(bundle, destination, target, version) {
  await mkdir(destination, { recursive: true });
  const artifacts = [];
  const packageName = (name) =>
    name.startsWith('Converter_') &&
    (name.endsWith('.dmg') || name.endsWith('-setup.exe') || name.endsWith('.app.zip'));
  async function walk(path) {
    for (const entry of await readdir(path, { withFileTypes: true })) {
      const source = join(path, entry.name);
      if (entry.isDirectory() && entry.name.endsWith('.app')) {
        const archive = join(destination, `Converter_${target}.app.zip`);
        const result = spawnSync(
          'ditto',
          ['-c', '-k', '--sequesterRsrc', '--keepParent', source, archive],
          {
            encoding: 'utf8',
          },
        );
        if (result.status !== 0) throw new Error(`ditto: ${result.stderr || result.error}`);
        artifacts.push(archive);
      } else if (entry.isDirectory()) await walk(source);
      else if (packageName(entry.name) && entry.name.startsWith(`Converter_${version}_`)) {
        const output = join(destination, entry.name);
        await copyFile(source, output);
        artifacts.push(output);
      }
    }
  }
  await walk(bundle);
  if (artifacts.length === 0) throw new Error(`Нативные пакеты версии ${version} не найдены`);
  // Папку забирает upload-artifact целиком: в ней не должно быть пакетов прошлого выпуска.
  for (const entry of await readdir(destination, { withFileTypes: true })) {
    const path = join(destination, entry.name);
    if (entry.isFile() && packageName(entry.name) && !artifacts.includes(path)) await rm(path);
  }
  return artifacts;
}
