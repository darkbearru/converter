import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { mkdir, readFile, readdir, copyFile, stat, writeFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { join, basename, resolve } from 'node:path';

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
await mkdir(destination, { recursive: true });
const artifacts = [];
async function walk(path) {
  for (const entry of await readdir(path, { withFileTypes: true })) {
    const source = join(path, entry.name);
    if (entry.isDirectory() && entry.name.endsWith('.app')) {
      const archive = join(destination, `Converter_${target}.app.zip`);
      run('ditto', ['-c', '-k', '--sequesterRsrc', '--keepParent', source, archive]);
      artifacts.push(archive);
    } else if (entry.isDirectory()) await walk(source);
    else if (entry.name.endsWith('.dmg') || entry.name.endsWith('-setup.exe')) {
      const output = join(destination, basename(source));
      await copyFile(source, output);
      artifacts.push(output);
    }
  }
}
await walk(join('target', target, 'release', 'bundle'));
if (artifacts.length === 0) throw new Error('Нативные пакеты не найдены');
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
]);
const info = {
  application: 'Converter',
  version: JSON.parse(await readFile('package.json', 'utf8')).version,
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
