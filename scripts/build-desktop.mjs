import { spawnSync } from 'node:child_process';
import { prepareBuildVersion } from './build-version.mjs';

const [target, ...options] = process.argv.slice(2);
const windows = target === 'x86_64-pc-windows-msvc';
if (!['aarch64-apple-darwin', 'x86_64-apple-darwin', 'x86_64-pc-windows-msvc'].includes(target))
  throw new Error('Нужен desktop target');
if (options.some((arg) => arg !== '--reuse-version'))
  throw new Error('Неизвестный параметр сборки');
if (process.platform !== (windows ? 'win32' : 'darwin'))
  throw new Error('Нативный пакет нужно собирать на целевой ОС');

console.log(
  `Версия сборки: ${await prepareBuildVersion(process.cwd(), options.includes('--reuse-version'))}`,
);
const args = ['build', '--target', target];
if (windows) args.push('--bundles', 'nsis', '--config', 'src-tauri/tauri.windows.conf.json');
args.push('--', '--locked');
// Запускаем JS CLI напрямую: одинаковое поведение аргументов на Windows и macOS.
for (const [script, parameters] of [
  ['node_modules/@tauri-apps/cli/tauri.js', args],
  ['scripts/collect-packages.mjs', [target]],
]) {
  const result = spawnSync(process.execPath, [script, ...parameters], { stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
