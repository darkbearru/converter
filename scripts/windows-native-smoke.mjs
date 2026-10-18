// Нативный WebView2 процесс из установленного NSIS, без подмены NativeBridge.
import { createServer } from 'node:http';
import { createHash } from 'node:crypto';
import { mkdir, writeFile, readFile, readdir } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import assert from 'node:assert/strict';

if (process.platform !== 'win32') throw new Error('Этот smoke запускается на Windows');
const application = resolve(process.argv[2]);
const output = resolve('.work/windows-native');
await mkdir(output, { recursive: true });
const payload = Buffer.from(
  '#NAME "Тест Windows"\nalpha\n\t[b]Пример[/b]\n' +
    Array.from(
      { length: 60000 },
      (_, i) =>
        `word${String(i).padStart(6, '0')}\n\tТестовая статья ${i}, проверка нативного приложения Windows.\n`,
    ).join(''),
);
const expectedSha = createHash('sha256').update(payload).digest('hex');
const requests = [];
const held = new Set();
let released = false;
const checkpoint = 1024 * 1024;
const server = createServer((request, response) => {
  const start = Number(/^bytes=(\d+)-$/.exec(request.headers.range ?? '')?.[1] ?? 0);
  requests.push({
    path: request.url,
    range: request.headers.range ?? null,
    if_range: request.headers['if-range'] ?? null,
  });
  const cancel = request.url === '/cancel';
  response.writeHead(start ? 206 : 200, {
    connection: 'close',
    'content-type': 'application/octet-stream',
    'content-disposition': 'attachment; filename="dictionary.dsl"',
    'content-length': payload.length - start,
    etag: '"native-v1"',
    ...(start ? { 'content-range': `bytes ${start}-${payload.length - 1}/${payload.length}` } : {}),
  });
  if (requests.length === 1) {
    response.end(payload.subarray(0, checkpoint));
    return;
  }
  if (released && !cancel) {
    response.end(payload.subarray(start));
    return;
  }
  const cut = Math.max(start, checkpoint);
  response.write(payload.subarray(start, cut));
  const item = { response, cut, cancel };
  held.add(item);
  response.on('close', () => held.delete(item));
});
await new Promise((resolveListen) => server.listen(0, '127.0.0.1', resolveListen));
const sourceUrl = `http://127.0.0.1:${server.address().port}/dictionary`;
const jobsRoot = join(process.env.APPDATA, 'dev.dictionary.converter', 'jobs');
async function readJobs() {
  return Promise.all(
    (await readdir(jobsRoot))
      .filter((f) => f.endsWith('.json'))
      .map(async (file) => JSON.parse(await readFile(join(jobsRoot, file), 'utf8'))),
  );
}
let session;
async function request(path, body, method = 'POST') {
  const response = await fetch(`http://127.0.0.1:4444${path}`, {
    method,
    ...(body === undefined
      ? {}
      : { headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) }),
    signal: AbortSignal.timeout(45000),
  });
  const json = await response.json();
  if (!response.ok || json.value?.error)
    throw new Error(
      `WebDriver ${json.value?.error ?? response.status}: ${json.value?.message ?? ''}`,
    );
  return json.value;
}
async function wait(condition, label, timeout = 30000) {
  const end = Date.now() + timeout;
  let last;
  while (Date.now() < end) {
    try {
      const value = await condition();
      if (value) return value;
    } catch (error) {
      last = error;
    }
    await new Promise((r) => setTimeout(r, 150));
  }
  throw new Error(`Timeout: ${label}${last ? `; ${last.message}` : ''}`);
}
const execute = (script, args = []) =>
  request(`/session/${session}/execute/sync`, { script, args });
async function clickText(text) {
  const element = await wait(
    () =>
      execute(
        'return Array.from(document.querySelectorAll("button")).find(b => b.textContent.trim() === arguments[0] && !b.disabled) || null',
        [text],
      ),
    text,
  );
  await request(
    `/session/${session}/element/${element['element-6066-11e4-a52e-4f735466cecf']}/click`,
    {},
  );
}
async function select(selector) {
  const element = await request(`/session/${session}/element`, {
    using: 'css selector',
    value: selector,
  });
  return element['element-6066-11e4-a52e-4f735466cecf'];
}
async function newSource(url) {
  await request(
    `/session/${session}/element/${await select('input[type="radio"][value="url"]')}/click`,
    {},
  );
  await request(`/session/${session}/element/${await select('input[type="url"]')}/value`, {
    text: url,
  });
  await clickText('Добавить и начать');
}
const hasText = (text) => execute('return document.body.innerText.includes(arguments[0])', [text]);
async function screenshot(name) {
  const base64 = await request(`/session/${session}/screenshot`, undefined, 'GET');
  await writeFile(join(output, name), Buffer.from(base64, 'base64'));
}
async function open() {
  const value = await request('/session', {
    capabilities: { alwaysMatch: { 'tauri:options': { application } } },
  });
  session = value.sessionId;
  await wait(() => hasText('Converter'), 'native frontend');
}
try {
  await wait(async () => (await fetch('http://127.0.0.1:4444/status')).ok, 'tauri-driver');
  await open();
  await newSource(sourceUrl);
  await wait(() => hasText('1 МиБ'), 'checkpoint');
  await clickText('Приостановить скачивание');
  await wait(() => hasText('Подтверждено и сохранено: 1 МиБ'), 'paused checkpoint');
  await screenshot('paused.png');
  await request(`/session/${session}`, undefined, 'DELETE');
  session = undefined;
  await wait(
    async () => {
      await open();
      return true;
    },
    'reopen application',
    45000,
  );
  await wait(() => hasText('Подтверждено и сохранено: 1 МиБ'), 'restored checkpoint');
  await clickText('Продолжить скачивание');
  await wait(
    () => requests.some((r) => r.range === `bytes=${checkpoint}-` && r.if_range === '"native-v1"'),
    'HTTP Range/If-Range',
  );
  released = true;
  for (const item of held) if (!item.cancel) item.response.end(payload.subarray(item.cut));
  await wait(() => hasText('dictionary.mdx'), 'conversion result', 90000);
  assert.equal(
    await execute(
      'return document.querySelector("[role=progressbar][aria-label=Скачивание]").getAttribute("aria-valuenow")',
    ),
    '100',
  );
  assert.equal(
    await execute(
      'return document.querySelector("[role=progressbar][aria-label=Конвертация]").getAttribute("aria-valuenow")',
    ),
    '100',
  );
  await screenshot('result.png');
  const beforeRepeat = requests.length;
  const firstJob = (await readJobs()).find(
    (item) => item.published && item.snapshot.state === 'succeeded',
  );
  assert.ok(firstJob);
  await clickText('Повторить конвертацию');
  await wait(
    async () =>
      (await readJobs()).some(
        (item) =>
          item.snapshot.id === firstJob.snapshot.id &&
          item.snapshot.generation > firstJob.snapshot.generation &&
          item.snapshot.state === 'succeeded',
      ),
    'persisted cached generation',
    90000,
  );
  await wait(() => hasText('dictionary.mdx'), 'cached conversion', 90000);
  assert.equal(requests.length, beforeRepeat, 'Повтор из кэша не должен создавать GET');
  const stored = await Promise.all(
    (await readdir(jobsRoot))
      .filter((f) => f.endsWith('.json'))
      .map(async (file) => JSON.parse(await readFile(join(jobsRoot, file), 'utf8'))),
  );
  const job = stored.find((item) => item.published && item.snapshot.state === 'succeeded');
  assert.ok(job, 'Готовое задание сохранено на диске');
  const report = JSON.parse(
    await readFile(join(job.published.directory, 'conversion-report.json'), 'utf8'),
  );
  assert.equal(report.sources[0].sha256, expectedSha);
  assert.equal(report.summary.source_articles, 60001);
  await clickText('Добавить');
  await newSource(sourceUrl.replace('/dictionary', '/cancel'));
  await wait(() => hasText('1 МиБ'), 'cancel checkpoint');
  await clickText('Отменить задание');
  await wait(() => hasText('Запустить заново'), 'cancelled');
  await writeFile(
    join(output, 'result.json'),
    JSON.stringify(
      {
        application,
        expected_sha256: expectedSha,
        bytes: payload.length,
        confirmed_before_close: checkpoint,
        source_articles: report.summary.source_articles,
        requests,
        cached_repeat_new_gets: 0,
        cancelled: true,
      },
      null,
      2,
    ),
  );
  console.log('Windows native smoke PASS: HTTP failure/pause/reopen/range/convert/cache/cancel');
} finally {
  if (session) await request(`/session/${session}`, undefined, 'DELETE').catch(() => {});
  for (const { response } of held) response.destroy();
  await new Promise((resolveClose) => server.close(resolveClose));
}
