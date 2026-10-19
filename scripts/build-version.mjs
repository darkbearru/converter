import { readFile, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { format, resolveConfig } from 'prettier';

export async function prepareBuildVersion(root = process.cwd(), reuse = false) {
  const paths = [
    'package.json',
    'package-lock.json',
    'src-tauri/tauri.conf.json',
    'Cargo.toml',
    'Cargo.lock',
  ];
  const contents = await Promise.all(paths.map((path) => readFile(join(root, path), 'utf8')));
  const [pkg, npmLock, tauri] = contents.slice(0, 3).map((text) => JSON.parse(text));
  const version = pkg.version;
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version))
    throw new Error('Ожидается версия major.minor.patch без суффикса');
  const workspace = /(\[workspace\.package\][\s\S]*?^version\s*=\s*")([^"]+)(")/m;
  const packages = contents[4].split('[[package]]');
  const localPackage = (block) =>
    /^name = "converter-[^"]+"$/m.test(block) && !/^source\s*=/m.test(block);
  const rustVersions = packages
    .filter(localPackage)
    .map((block) => block.match(/^version = "([^"]+)"$/m)?.[1]);
  const versions = [
    npmLock.version,
    npmLock.packages?.['']?.version,
    tauri.version,
    contents[3].match(workspace)?.[2],
    ...rustVersions,
  ];
  if (!rustVersions.length || versions.some((value) => value !== version))
    throw new Error('Версии npm, Tauri и Rust расходятся; сначала синхронизируйте версии');
  if (reuse) return version;
  const [major, minor, patch] = version.split('.');
  const next = `${major}.${minor}.${BigInt(patch) + 1n}`;
  pkg.version = npmLock.version = npmLock.packages[''].version = tauri.version = next;
  const formatting = await resolveConfig(join(root, 'package.json'));
  const updated = [
    ...(await Promise.all(
      [pkg, npmLock, tauri].map((value, index) =>
        index === 1
          ? `${JSON.stringify(value, null, 2)}\n`
          : format(JSON.stringify(value), { ...formatting, filepath: join(root, paths[index]) }),
      ),
    )),
    contents[3].replace(workspace, (_, before, _old, after) => `${before}${next}${after}`),
    packages
      .map((block) =>
        localPackage(block) ? block.replace(/^version = "[^"]+"$/m, `version = "${next}"`) : block,
      )
      .join('[[package]]'),
  ];
  // Сначала проверены все файлы: ошибка версии не оставляет частично изменённые манифесты.
  for (let index = 0; index < paths.length; index++)
    await writeFile(join(root, paths[index]), updated[index]);
  return next;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv.slice(2).some((arg) => arg !== '--reuse'))
    throw new Error('Поддерживается только --reuse для подготовленной версии');
  console.log(
    `Версия сборки: ${await prepareBuildVersion(process.cwd(), process.argv.includes('--reuse'))}`,
  );
}
