import { mkdir, copyFile, writeFile } from 'node:fs/promises';
import { chromium, webkit } from '@playwright/test';
import { createServer } from 'vite';

const server = await createServer({ server: { host: '127.0.0.1', port: 1421, strictPort: true } });
await server.listen();
await mkdir('.impeccable/review', { recursive: true });
await mkdir('docs/validation/ui', { recursive: true });
const observations = [];
try {
  for (const [engine, browserType] of [
    ['chromium', chromium],
    ['webkit', webkit],
  ]) {
    const browser = await browserType.launch();
    try {
      for (const [size, viewport] of [
        ['desktop', { width: 1280, height: 800 }],
        ['mobile', { width: 390, height: 844 }],
      ]) {
        const page = await browser.newPage({ viewport });
        await page.goto(
          'http://127.0.0.1:1421/tests/ui/harness/index.html?scenario=converting&capture=1',
        );
        await page.getByText('Запись MDX', { exact: true }).waitFor();
        const name = `${size}${engine === 'webkit' ? '-webkit' : ''}.png`;
        await page.screenshot({
          path: `.impeccable/review/${name}`,
          fullPage: true,
          animations: 'disabled',
        });
        await copyFile(`.impeccable/review/${name}`, `docs/validation/ui/${name}`);
        observations.push(
          await page.evaluate(
            ({ engine, viewport }) => {
              const text = document.querySelector('.phase h3');
              const phase = document.querySelector('.phase');
              return {
                engine,
                viewport,
                documentWidth: document.documentElement.scrollWidth,
                documentHeight: document.documentElement.scrollHeight,
                h3: text
                  ? { size: getComputedStyle(text).fontSize, color: getComputedStyle(text).color }
                  : null,
                phasePadding: phase ? getComputedStyle(phase).padding : null,
                download: document
                  .querySelector('[aria-label="Скачивание"][role="progressbar"]')
                  ?.getAttribute('aria-valuenow'),
                conversion: document
                  .querySelector('[aria-label="Конвертация"][role="progressbar"]')
                  ?.getAttribute('aria-valuenow'),
              };
            },
            { engine, viewport },
          ),
        );
        await page.close();
      }
    } finally {
      await browser.close();
    }
  }
  await writeFile(
    'docs/validation/ui/measurements.json',
    `${JSON.stringify(observations, null, 2)}\n`,
  );
  console.log('Сохранены 4 снимка Chromium/WebKit и измерения двух прогрессов');
} finally {
  await server.close();
}
