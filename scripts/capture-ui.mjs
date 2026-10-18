import { mkdir, copyFile, readFile, writeFile } from 'node:fs/promises';
import { chromium, webkit } from '@playwright/test';
import { createServer } from 'vite';

const layout = process.argv.includes('--layout');
const history = process.argv.includes('--history');
const compact = process.argv.includes('--compact');
const diagnostics = process.argv.includes('--diagnostics');
const group = diagnostics
  ? 'diagnostics-dialog'
  : compact
    ? 'compact-warning'
    : history
      ? 'history-actions'
      : layout
        ? 'composer-layout'
        : 'composer';
const review = `.impeccable/review/${group}`;
const archive = `docs/validation/ui/${group}`;
const only = process.argv.find((arg) => arg.startsWith('--only='))?.slice(7);
const cases = diagnostics
  ? [
      ['link-dark', 'webkit', 'dark', 'warning', 'history'],
      ['dialog-dark', 'webkit', 'dark', 'warning', 'diagnostics'],
      [
        'dialog-small-light',
        'chromium',
        'light',
        'warning',
        'diagnostics',
        { width: 540, height: 400 },
      ],
    ]
  : compact
    ? [
        ['empty-dark', 'webkit', 'dark', ''],
        ['warning-dark', 'webkit', 'dark', 'warning', 'history'],
        [
          'warning-small-light',
          'chromium',
          'light',
          'warning',
          'history',
          { width: 540, height: 400 },
        ],
      ]
    : history
      ? [
          ['warning-dark', 'webkit', 'dark', 'warning', 'history'],
          [
            'actions-small-light',
            'webkit',
            'light',
            'succeeded',
            'history',
            { width: 540, height: 400 },
          ],
          ['delete-light', 'chromium', 'light', 'succeeded', 'delete'],
          ['delete-small-dark', 'webkit', 'dark', 'long', 'delete', { width: 540, height: 400 }],
          ['deleted-light', 'chromium', 'light', 'succeeded', 'deleted'],
        ]
      : layout
        ? [
            ['empty-dark', 'chromium', 'dark', ''],
            ['one-dark', 'chromium', 'dark', 'history-one'],
            ['many-dark', 'chromium', 'dark', 'history-many'],
            ['active-light', 'chromium', 'light', 'history-active'],
            ['small-light', 'webkit', 'light', 'history-many', null, { width: 540, height: 400 }],
            [
              'download-small',
              'webkit',
              'light',
              'history-many',
              'download',
              { width: 540, height: 400 },
            ],
          ]
        : [
            ['empty-dark', 'chromium', 'dark', ''],
            ['download-dark', 'chromium', 'dark', 'downloading'],
            ['ready-dark', 'chromium', 'dark', 'ready'],
            ['active-dark', 'chromium', 'dark', 'converting'],
            ['stack-dark', 'chromium', 'dark', 'stack'],
            ['settings-dark', 'chromium', 'dark', '', 'settings'],
            ['settings-resources', 'chromium', 'dark', 'ready-resources', 'settings'],
            ['link-dark', 'chromium', 'dark', '', 'link'],
            ['history-dark', 'chromium', 'dark', 'succeeded', 'history'],
            ['empty-light', 'chromium', 'light', ''],
            ['stack-light', 'chromium', 'light', 'stack'],
            ['history-light', 'chromium', 'light', 'long', 'history'],
            ['stack-webkit-light', 'webkit', 'light', 'stack'],
            ['download-webkit-dark', 'webkit', 'dark', 'downloading'],
            ['settings-small', 'webkit', 'light', '', 'settings', { width: 540, height: 400 }],
          ];
const server = await createServer({ server: { host: '127.0.0.1', port: 1422, strictPort: true } });
await server.listen();
await Promise.all([mkdir(review, { recursive: true }), mkdir(archive, { recursive: true })]);
const observations = [];
try {
  for (const engine of ['chromium', 'webkit']) {
    const browser = await { chromium, webkit }[engine].launch();
    try {
      for (const [name, caseEngine, colorScheme, scenario, action, size] of cases) {
        if (caseEngine !== engine) continue;
        if (only && name !== only) continue;
        const viewport = size ?? { width: 740, height: 500 };
        const page = await browser.newPage({ viewport, colorScheme, reducedMotion: 'reduce' });
        const errors = [];
        page.on('pageerror', (error) => errors.push(error.message));
        await page.goto(
          `http://127.0.0.1:1422/tests/ui/harness/index.html?scenario=${scenario}&capture=1`,
        );
        await page.locator('.source-composer').waitFor();
        if (scenario.startsWith('history-'))
          await page.waitForFunction(
            (count) => document.querySelectorAll('.p-accordionheader').length === count,
            scenario === 'history-one' ? 1 : 12,
          );
        if (scenario === 'history-active')
          await page.waitForFunction(() => document.querySelectorAll('.active-task').length === 2);
        if (scenario === 'stack')
          await page.waitForFunction(() => document.querySelectorAll('.active-task').length === 2);
        if (scenario === 'converting') await page.locator('.active-task').waitFor();
        if (['ready', 'ready-resources'].includes(scenario))
          await page.getByRole('button', { name: 'Конвертировать', exact: true }).waitFor();
        if (scenario === 'downloading')
          await page.getByRole('progressbar', { name: 'Скачивание', exact: true }).waitFor();
        if (action === 'settings')
          await page.getByRole('button', { name: 'Настройки конвертации', exact: true }).click();
        if (action === 'link')
          await page.getByRole('button', { name: 'Добавить ссылку', exact: true }).click();
        if (['history', 'delete', 'deleted', 'diagnostics'].includes(action)) {
          await page.locator('.p-accordionheader').click();
          await page.locator('.result-path').waitFor();
          if (['warning', 'long'].includes(scenario))
            await page.locator('.diagnostic-list li').first().waitFor();
        }
        if (action === 'diagnostics') {
          await page.getByRole('link', { name: /ещё 28\s172 предупреждения/ }).click();
          await page.locator('.diagnostics-dialog .diagnostic-list li').last().waitFor();
        }
        if (['delete', 'deleted'].includes(action)) {
          await page.getByRole('button', { name: 'Удалить', exact: true }).click();
          if (action === 'deleted') {
            await page
              .getByRole('dialog')
              .getByRole('button', { name: 'Удалить', exact: true })
              .click();
            await page.getByRole('dialog').waitFor({ state: 'hidden' });
            await page.locator('.completed-history').waitFor({ state: 'hidden' });
          }
        }
        if (action === 'download') {
          await page.getByRole('button', { name: 'Добавить ссылку', exact: true }).click();
          await page
            .getByLabel('Ссылка на словарь', { exact: true })
            .fill('https://example.org/main.dsl');
          await page.getByRole('button', { name: 'Скачать', exact: true }).click();
          await page.getByRole('progressbar', { name: 'Скачивание', exact: true }).waitFor();
          await page.getByRole('dialog').waitFor({ state: 'hidden' });
        }
        await page.evaluate(async () => {
          await document.fonts.ready;
        });
        if (compact && viewport.width === 540) {
          await page.locator('.completed-history').evaluate(async (history) => {
            await Promise.allSettled(
              history.getAnimations({ subtree: true }).map((animation) => animation.finished),
            );
            const actions = history.querySelector('.history-actions');
            if (actions)
              history.scrollTop += Math.max(
                0,
                actions.getBoundingClientRect().bottom - history.getBoundingClientRect().bottom + 8,
              );
          });
        }
        await page.screenshot({ path: `${review}/${name}.png`, animations: 'disabled' });
        await copyFile(`${review}/${name}.png`, `${archive}/${name}.png`);
        const bounds = await page.evaluate(() => {
          const rect = (selector) => {
            const el = document.querySelector(selector);
            if (!el) return null;
            const { x, y, width, height } = el.getBoundingClientRect();
            return { x, y, width, height };
          };
          return {
            documentWidth: document.documentElement.scrollWidth,
            documentHeight: document.documentElement.scrollHeight,
            canvas: getComputedStyle(document.body).backgroundColor,
            text: getComputedStyle(document.body).color,
            composer: rect('.source-composer'),
            workspace: rect('.conversion-workspace'),
            workspacePaddingBottom: getComputedStyle(
              document.querySelector('.conversion-workspace'),
            ).paddingBottom,
            history: rect('.completed-history'),
            jobPanels: rect('.job-panels'),
            stack: rect('.task-stack'),
            dialog: rect('.p-dialog'),
            diagnosticsList: rect('.diagnostics-dialog .diagnostic-list'),
            diagnosticsRows: document.querySelectorAll('.diagnostics-dialog .diagnostic-list li')
              .length,
            diagnosticsLink: [...document.querySelectorAll('.diagnostics-more')].map((el) => ({
              color: getComputedStyle(el).color,
              underline: getComputedStyle(el).textDecorationStyle,
              tag: el.tagName,
            })),
            toolbar: [...document.querySelectorAll('.composer-tools .p-button')].map((el) => {
              const icon = el.querySelector('.app-icon');
              const r = icon.getBoundingClientRect();
              const b = el.getBoundingClientRect();
              return {
                label: el.getAttribute('aria-label'),
                x: b.x,
                y: b.y,
                width: b.width,
                height: b.height,
                iconWidth: r.width,
                iconHeight: r.height,
                border: getComputedStyle(el).borderTopWidth,
                gap: getComputedStyle(el.parentElement).gap,
              };
            }),
            historyActions: [...document.querySelectorAll('.history-actions .p-button')].map(
              (el) => {
                const { x, y, width, height } = el.getBoundingClientRect();
                return {
                  label: el.textContent,
                  x,
                  y,
                  width,
                  height,
                  color: getComputedStyle(el).color,
                  background: getComputedStyle(el).backgroundColor,
                };
              },
            ),
            dialogButtons: [...document.querySelectorAll('.p-dialog .p-button')].map((el) => ({
              label: el.textContent,
              color: getComputedStyle(el).color,
              background: getComputedStyle(el).backgroundColor,
              dialogBackground: getComputedStyle(el.closest('.p-dialog')).backgroundColor,
            })),
            progress: [...document.querySelectorAll('[role="progressbar"]')].map((el) => ({
              label: el.getAttribute('aria-label'),
              value: el.getAttribute('aria-valuenow'),
              background: getComputedStyle(el).backgroundColor,
              fill: getComputedStyle(el.querySelector('.p-progressbar-value')).backgroundColor,
            })),
          };
        });
        observations.push({ name, engine, colorScheme, viewport, errors, ...bounds });
        await page.close();
      }
    } finally {
      await browser.close();
    }
  }
  const previous = only ? JSON.parse(await readFile(`${archive}/measurements.json`, 'utf8')) : [];
  const measurements = [
    ...previous.filter((item) => !observations.some((current) => current.name === item.name)),
    ...observations,
  ];
  await writeFile(`${archive}/measurements.json`, `${JSON.stringify(measurements, null, 2)}\n`);
  console.log(
    `Сохранены ${observations.length} снимков состояний 740×500 и 540×400; ошибок JavaScript: ${observations.reduce((n, item) => n + item.errors.length, 0)}`,
  );
} finally {
  await server.close();
}
