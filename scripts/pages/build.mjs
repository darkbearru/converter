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
const config = JSON.parse(await readFile(join(root, 'config/locales.json'), 'utf8'));
const languages = Object.keys(config.locales);
const template = await readFile(join(root, 'website/index.html'), 'utf8');
const escape = (value) =>
  String(value).replace(
    /[&<>"']/g,
    (character) =>
      ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[character],
  );
const base = process.env.PAGES_BASE_URL ?? 'https://darkbearru.github.io/converter/';
const baseUrl = new URL(base);
if (
  baseUrl.protocol !== 'https:' ||
  baseUrl.username ||
  baseUrl.password ||
  baseUrl.search ||
  baseUrl.hash ||
  !base.endsWith('/')
)
  throw new Error('PAGES_BASE_URL must be an HTTPS directory URL without credentials or query');
const pages = [];
for (const [directory, locale] of [['', 'en'], ...languages.map((locale) => [locale, locale])]) {
  const copy = JSON.parse(await readFile(join(root, `website/locales/${locale}.json`), 'utf8'));
  const prefix = directory ? '../' : '';
  const image = locale === 'ru' ? 'interface.jpg' : `interface-${locale}.jpg`;
  const size = (bytes) =>
    `${new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(bytes / 1_000_000)} ${locale === 'ru' ? 'Мб' : 'MB'}`;
  const support = config.locales[locale].support_url;
  if (support !== null && new URL(support).protocol !== 'https:')
    throw new Error('Support URL must use HTTPS');
  const supportLink = (className) =>
    support
      ? `<a class="${className}" href="${escape(support)}"><img src="${prefix}126__cash.svg" alt="" width="18" height="18" />${escape(copy.thanks)}</a>`
      : '';
  const values = {
    VERSION: version,
    MAC_URL: mac.url,
    WIN_URL: windows.url,
    MAC_SIZE: size(mac.bytes),
    WIN_SIZE: size(windows.bytes),
    LANG: locale,
    PAGE_URL: base + locale + '/',
    IMAGE_URL: base + image,
    IMAGE_FILE: image,
    IMAGE_WIDTH: locale === 'ru' ? 1280 : 740,
    IMAGE_HEIGHT: locale === 'ru' ? 720 : 500,
    ASSET_PREFIX: prefix,
    LANGUAGE_ROOT: String(!directory),
    SUPPORTED_LANGUAGES: languages.join(','),
    ALTERNATE_LINKS: [
      ...languages.map(
        (language) =>
          `<link rel="alternate" hreflang="${escape(language)}" href="${base}${escape(language)}/" />`,
      ),
      `<link rel="alternate" hreflang="x-default" href="${base}" />`,
    ].join('\n    '),
    LANGUAGE_LINKS: `<nav class="site-languages" aria-label="${escape(copy.language)}">${languages.map((language) => `<a href="${prefix}${language}/" data-site-language="${escape(language)}" lang="${escape(language)}"${language === locale ? ' aria-current="page"' : ''}>${escape(config.locales[language].label)}</a>`).join('')}</nav>`,
    SUPPORT_HEADER: supportLink('source-link important-link'),
    SUPPORT_FOOTER: supportLink('important-link'),
  };
  const html = template.replace(/\{\{(TEXT\.)?([A-Z_a-z]+)\}\}/g, (_, text, key) => {
    const value = text ? copy[key] : values[key];
    if (value === undefined) throw new Error(`Missing page value: ${locale}/${key}`);
    return !text &&
      ['ALTERNATE_LINKS', 'LANGUAGE_LINKS', 'SUPPORT_HEADER', 'SUPPORT_FOOTER'].includes(key)
      ? value
      : escape(value);
  });
  if (/\{\{[^}]+\}\}/.test(html)) throw new Error('Не все данные страницы заполнены');
  pages.push({ directory, html });
}
await mkdir(output, { recursive: true });
for (const name of [
  'style.css',
  'language.js',
  'interface.jpg',
  'interface-en.jpg',
  '126__cash.svg',
])
  await copyFile(join(root, 'website', name), join(output, name));
await copyFile(join(root, 'book.svg'), join(output, 'book.svg'));
await copyFile(join(root, 'src/assets/icons/027__download.svg'), join(output, 'download.svg'));
await writeFile(
  join(output, 'release.json'),
  JSON.stringify({ version, macos: mac, windows }, null, 2) + '\n',
);
for (const { directory, html } of pages) {
  await mkdir(join(output, directory), { recursive: true });
  await writeFile(join(output, directory, 'index.html'), html);
}
console.log(`Страница Converter ${version}: ${output}`);
