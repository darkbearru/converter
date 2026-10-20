let input = '';
for await (const chunk of process.stdin) input += chunk;
const pages = JSON.parse(input);
if (!Array.isArray(pages)) throw new Error('Ожидается список релизов GitHub');
const seen = new Set();
let total = 0;
for (const release of pages.flat()) {
  if (!release || !Number.isSafeInteger(release.id) || typeof release.draft !== 'boolean')
    throw new Error('Некорректный релиз GitHub');
  if (release.draft || seen.has(release.id)) continue;
  seen.add(release.id);
  if (!Array.isArray(release.assets)) throw new Error('Нет списка файлов релиза');
  for (const asset of release.assets) {
    if (!/^Converter_.+\.(?:dmg|exe)$/i.test(asset.name) || asset.state !== 'uploaded') continue;
    if (!Number.isSafeInteger(asset.download_count) || asset.download_count < 0)
      throw new Error('Некорректное число скачиваний');
    total += asset.download_count;
    if (!Number.isSafeInteger(total)) throw new Error('Переполнение суммы скачиваний');
  }
}
console.log(JSON.stringify({ total }, null, 2));
