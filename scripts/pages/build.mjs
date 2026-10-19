import { copyFile, mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../../', import.meta.url));
const args = process.argv.slice(2);
if (args.length && (args.length !== 2 || args[0] !== '--output'))
  throw new Error('Укажите --output <папка>');
const output = resolve(args[1] ?? join(root, '.work/pages/site'));
let input = '';
for await (const chunk of process.stdin) input += chunk;
const release = JSON.parse(input);
const match = /^(?:v)?((?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*))$/.exec(
  release.tag_name ?? '',
);
if (
  !match ||
  release.draft !== false ||
  release.prerelease !== false ||
  !Array.isArray(release.assets)
) {
  throw new Error('Нужен опубликованный стабильный релиз с версией major.minor.patch');
}
const version = match[1];
function installer(name) {
  const matches = release.assets.filter((asset) => asset.name === name);
  if (matches.length !== 1) throw new Error(`Не найден единственный установщик ${name}`);
  const asset = matches[0];
  const url = `https://github.com/darkbearru/converter/releases/download/${release.tag_name}/${name}`;
  if (
    asset.state !== 'uploaded' ||
    asset.browser_download_url !== url ||
    !Number.isSafeInteger(asset.size) ||
    asset.size <= 0
  ) {
    throw new Error(`Некорректный установщик ${name}`);
  }
  return { url, bytes: asset.size };
}
const mac = installer(`Converter_${version}_aarch64.dmg`);
const windows = installer(`Converter_${version}_x64-setup.exe`);
const megabytes = (bytes) =>
  `${new Intl.NumberFormat('ru-RU', { maximumFractionDigits: 1 }).format(bytes / 1_000_000)} Мб`;
const values = {
  VERSION: version,
  MAC_URL: mac.url,
  WIN_URL: windows.url,
  MAC_SIZE: megabytes(mac.bytes),
  WIN_SIZE: megabytes(windows.bytes),
};
let html = await readFile(join(root, 'website/index.html'), 'utf8');
for (const [key, value] of Object.entries(values)) html = html.replaceAll(`{{${key}}}`, value);
if (/\{\{[^}]+\}\}/.test(html)) throw new Error('Не все данные страницы заполнены');
await mkdir(output, { recursive: true });
for (const name of ['style.css', 'interface.jpg'])
  await copyFile(join(root, 'website', name), join(output, name));
await copyFile(join(root, 'book.svg'), join(output, 'book.svg'));
await copyFile(join(root, 'src/assets/icons/027__download.svg'), join(output, 'download.svg'));
await writeFile(
  join(output, 'release.json'),
  JSON.stringify({ version, macos: mac, windows }, null, 2) + '\n',
);
await writeFile(join(output, 'index.html'), html);
console.log(`Страница Converter ${version}: ${output}`);
