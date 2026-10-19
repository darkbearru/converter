import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { readFile, stat, writeFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { join, basename, resolve } from 'node:path';
import { collectPackageFiles } from './package-files.mjs';

const target = process.argv[2];
if (!['aarch64-apple-darwin', 'x86_64-apple-darwin', 'x86_64-pc-windows-msvc'].includes(target))
  throw new Error('Нужен desktop target');
function run(program, args) {
  const result = spawnSync(program, args, {
    encoding: 'utf8',
    shell: process.platform === 'win32' && program === 'npm',
  });
  if (result.status !== 0) throw new Error(`${program}: ${result.stderr || result.error}`);
  return result.stdout.trim();
}
async function sha256(path) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest('hex');
}
const destination = resolve('.work/packages', target);
const version = JSON.parse(await readFile('package.json', 'utf8')).version;
const artifacts = await collectPackageFiles(
  join('target', target, 'release', 'bundle'),
  destination,
  target,
  version,
);
const lockfiles = {};
for (const name of ['Cargo.lock', 'package-lock.json', 'rust-toolchain.toml'])
  lockfiles[name] = await sha256(name);
const dirty = run('git', [
  'status',
  '--porcelain',
  '--untracked-files=no',
  '--',
  'src',
  'src-tauri',
  'crates',
  'Cargo.toml',
  'Cargo.lock',
  'package.json',
  'package-lock.json',
  'vite.config.ts',
  'rust-toolchain.toml',
  'scripts',
]);
const info = {
  application: 'Converter',
  version,
  commit: run('git', ['rev-parse', 'HEAD']),
  source_dirty: Boolean(dirty),
  target,
  created_at: new Date().toISOString(),
  toolchain: {
    node: process.version,
    npm: run('npm', ['--version']),
    rustc: run('rustc', ['--version']),
    cargo: run('cargo', ['--version']),
  },
  lockfiles,
  artifacts: await Promise.all(
    artifacts.map(async (path) => ({
      file: basename(path),
      bytes: (await stat(path)).size,
      sha256: await sha256(path),
    })),
  ),
};
await writeFile(join(destination, 'build-info.json'), `${JSON.stringify(info, null, 2)}\n`);
await writeFile(
  join(destination, 'SHA256SUMS'),
  info.artifacts.map((a) => `${a.sha256}  ${a.file}`).join('\n') + '\n',
);
console.log(
  `Сохранено пакетов: ${artifacts.length}, target: ${target}, commit: ${info.commit}, source_dirty: ${info.source_dirty}`,
);
