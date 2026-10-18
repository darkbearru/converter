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

it('запоздалая ошибка прежнего пути не отменяет новый запрос', async () => {
  const job = structuredClone(fixtures.succeeded) as JobSnapshot;
  let rejectOld!: (reason: Error) => void;
  let resolveNew!: (value: string) => void;
  const native = {
    getResultPath: vi
      .fn()
      .mockImplementationOnce(
        () =>
          new Promise((_, reject) => {
            rejectOld = reject;
          }),
      )
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveNew = resolve;
          }),
      ),
  } as unknown as NativeBridge;
  const view = render(CompletedHistory, { props: { jobs: [job], bridge: native, pending: {} } });
  await fireEvent.click(screen.getByRole('button', { name: new RegExp(job.display_name) }));
  const updated = structuredClone(job);
  updated.result!.id = 'new-result';
  await view.rerender({ jobs: [updated] });
  await waitFor(() => expect(native.getResultPath).toHaveBeenCalledTimes(2));
  rejectOld(new Error('Старый результат недоступен'));
  await Promise.resolve();
  resolveNew('/new/result');
  expect(await screen.findByRole('button', { name: 'Открыть папку /new/result' })).toBeVisible();
  expect(screen.queryByText('Старый результат недоступен')).not.toBeInTheDocument();
});

it('разворачивает историю с файлами, предупреждением и активным путём', async () => {
  const job = structuredClone(fixtures.succeeded) as JobSnapshot;
  job.had_errors = true;
  const native = {
    getResultPath: vi.fn(async () => '/Словари/main-converted'),
    readDiagnostics: vi.fn(async () => ({ items: [], total: 0, next_offset: null })),
  } as unknown as NativeBridge;
  const open = vi.fn();
  render(CompletedHistory, { props: { jobs: [job], bridge: native, pending: {}, onOpen: open } });
  expect(screen.getByLabelText('Были предупреждения или ошибки')).toBeInTheDocument();
  await fireEvent.click(screen.getByRole('button', { name: new RegExp(job.display_name) }));
  const path = await screen.findByRole('button', { name: 'Открыть папку /Словари/main-converted' });
  expect(screen.getByText('dictionary.mdx')).toBeVisible();
  await fireEvent.click(path);
  await waitFor(() => expect(open).toHaveBeenCalledWith(job.id));
});

it('обновляет путь после повторной конвертации того же задания', async () => {
  const job = structuredClone(fixtures.succeeded) as JobSnapshot;
  const native = {
    getResultPath: vi
      .fn()
      .mockResolvedValueOnce('/old/result')
      .mockResolvedValueOnce('/new/result'),
  } as unknown as NativeBridge;
  const view = render(CompletedHistory, { props: { jobs: [job], bridge: native, pending: {} } });
  await fireEvent.click(screen.getByRole('button', { name: new RegExp(job.display_name) }));
  expect(await screen.findByRole('button', { name: 'Открыть папку /old/result' })).toBeVisible();
  const updated = structuredClone(job);
  updated.result!.id = 'new-result';
  await view.rerender({ jobs: [updated] });
  expect(await screen.findByRole('button', { name: 'Открыть папку /new/result' })).toBeVisible();
  expect(
    screen.queryByRole('button', { name: 'Открыть папку /old/result' }),
  ).not.toBeInTheDocument();
});
