import { expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/vue';
import CompletedHistory from '../../src/features/jobs/CompletedHistory.vue';
import fixtures from './fixtures/contracts.json';
import type { JobSnapshot, NativeBridge } from '../../src/shared/contracts';

it('сразу показывает текст предупреждения в раскрытой записи', async () => {
  const job = structuredClone(fixtures.succeeded) as JobSnapshot;
  job.state = 'succeeded_with_warnings';
  job.diagnostic_count = 28173;
  const native = {
    getResultPath: async () => '/result',
    readDiagnostics: async () => ({
      items: [
        {
          code: 'missing_reference',
          message: 'Не найдена статья для ссылки: 儿',
          source_location: { source: job.display_name, line: 4 },
        },
      ],
      total: 28173,
      next_offset: 1,
    }),
  } as unknown as NativeBridge;
  render(CompletedHistory, { props: { jobs: [job], bridge: native, pending: {} } });
  await fireEvent.click(screen.getByRole('button', { name: new RegExp(job.display_name) }));
  expect(await screen.findByText('Не найдена статья для ссылки: 儿')).toBeVisible();
  expect(screen.getByText(/ещё 28\s172 предупреждения/)).toBeVisible();
  expect(
    screen.getByText('Не найдена статья для ссылки: 儿').closest('.p-message-warn'),
  ).toBeInTheDocument();
  expect(screen.queryByText('Готово с предупреждениями')).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: /Показать все/ })).not.toBeInTheDocument();
});

it('ссылка остальных предупреждений открывает весь журнал с переходом к последней странице', async () => {
  const job = structuredClone(fixtures.succeeded) as JobSnapshot;
  job.diagnostic_count = 123;
  const native = {
    getResultPath: async () => '/result',
    readDiagnostics: vi.fn(async (_id: string, offset: number, limit: number) => ({
      items: Array.from({ length: Math.min(limit, 123 - offset) }, (_, index) => ({
        code: 'missing_reference',
        message: `Предупреждение ${offset + index + 1}`,
        source_location: null,
      })),
      total: 123,
      next_offset: offset + limit < 123 ? offset + limit : null,
    })),
  } as unknown as NativeBridge;
  render(CompletedHistory, { props: { jobs: [job], bridge: native, pending: {} } });
  await fireEvent.click(screen.getByRole('button', { name: new RegExp(job.display_name) }));
  const link = await screen.findByRole('link', { name: '(ещё 122 предупреждения)' });
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  expect(native.readDiagnostics).toHaveBeenLastCalledWith(job.id, 0, 1);
  await fireEvent.click(link);
  const dialog = await screen.findByRole('dialog', { name: 'Предупреждения' });
  expect(within(dialog).getByText(job.display_name)).toBeVisible();
  expect(await within(dialog).findByText('Предупреждение 50')).toBeVisible();
  expect(within(dialog).queryByText('Предупреждение 51')).not.toBeInTheDocument();
  expect(native.readDiagnostics).toHaveBeenLastCalledWith(job.id, 0, 50);
  await fireEvent.click(within(dialog).getByRole('button', { name: 'Последняя страница' }));
  expect(await within(dialog).findByText('Предупреждение 123')).toBeVisible();
  expect(within(dialog).queryByText('Предупреждение 50')).not.toBeInTheDocument();
  expect(native.readDiagnostics).toHaveBeenLastCalledWith(job.id, 100, 50);
  await fireEvent.click(within(dialog).getAllByRole('button', { name: 'Закрыть' }).at(-1)!);
  await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  expect(link).toBeVisible();
});

it('готовый результат показывает файлы и одну кнопку открытия папки без запроса отдельного пути', async () => {
  const job = structuredClone(fixtures.succeeded) as JobSnapshot;
  const native = { getResultPath: vi.fn() } as unknown as NativeBridge;
  const open = vi.fn();
  render(CompletedHistory, { props: { jobs: [job], bridge: native, pending: {}, onOpen: open } });
  await fireEvent.click(screen.getByRole('button', { name: new RegExp(job.display_name) }));
  expect(screen.getByText('dictionary.mdx')).toBeVisible();
  expect(screen.queryByText('Готово')).not.toBeInTheDocument();
  expect(screen.queryByRole('heading', { name: 'Результат' })).not.toBeInTheDocument();
  const buttons = screen.getAllByRole('button', { name: /^Открыть папку/ });
  expect(buttons).toHaveLength(1);
  await fireEvent.click(buttons[0]!);
  expect(open).toHaveBeenCalledWith(job.id);
  expect(native.getResultPath).not.toHaveBeenCalled();
});

it('ошибка конвертации и её журнал используют сообщения уровня error', async () => {
  const job = structuredClone(fixtures.succeeded) as JobSnapshot;
  job.state = 'failed';
  job.result = null;
  job.error = { code: 'invalid_markup', message: 'Нарушена структура статьи' };
  job.diagnostic_count = 1;
  const native = {
    readDiagnostics: async () => ({
      items: [{ code: 'unknown_tag', message: 'Неизвестный тег', source_location: null }],
      total: 1,
      next_offset: null,
    }),
  } as unknown as NativeBridge;
  render(CompletedHistory, { props: { jobs: [job], bridge: native, pending: {} } });
  await fireEvent.click(screen.getByRole('button', { name: new RegExp(job.display_name) }));
  const diagnostic = await screen.findByText('Неизвестный тег');
  expect(diagnostic.closest('.p-message-error')).toBeInTheDocument();
  expect(
    screen.getByText('Нарушена структура статьи').closest('.p-message-error'),
  ).toBeInTheDocument();
});
