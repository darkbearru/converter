import { expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/vue';
import DiagnosticsList from '../../src/features/results/DiagnosticsList.vue';
import type { DiagnosticPage, NativeBridge } from '../../src/shared/contracts';

it('группа показывает число употреблений отдельно от количества предупреждений', async () => {
  const native = {
    readDiagnostics: async () => ({
      items: [
        {
          code: 'missing_resource',
          message: 'Отсутствует ресурс: voice.wav',
          severity: 'warning',
          occurrences: 1234,
          source_location: { source: 'test.dsl', line: 7 },
        },
      ],
      total: 2,
      next_offset: 1,
    }),
  } as unknown as NativeBridge;
  render(DiagnosticsList, { props: { id: 'job', count: 2, bridge: native, preview: true } });
  expect(await screen.findByText('Отсутствует ресурс: voice.wav')).toBeVisible();
  expect(screen.getByText(/Встречается 1\s234 раза/)).toBeVisible();
  expect(screen.getByText(/Первое упоминание: test.dsl, строка 7/)).toBeVisible();
  expect(screen.getByRole('link', { name: '(ещё 1 предупреждение)' })).toBeVisible();
});

it('старый журнал без счётчика употреблений остаётся читаемым', async () => {
  const native = {
    readDiagnostics: async () => ({
      items: [{ code: 'test', message: 'Старая запись', source_location: null }],
      total: 1,
      next_offset: null,
    }),
  } as unknown as NativeBridge;
  render(DiagnosticsList, { props: { id: 'legacy', count: 1, bridge: native, expanded: true } });
  expect(await screen.findByText('Старая запись')).toBeVisible();
  expect(screen.getByText('Старая запись').closest('.p-message')).not.toBeInTheDocument();
  expect(screen.queryByText(/Встречается/)).not.toBeInTheDocument();
});

it('ошибка чтения полного журнала предлагает повтор', async () => {
  const native = {
    readDiagnostics: vi
      .fn()
      .mockRejectedValueOnce(new Error('Не удалось прочитать журнал'))
      .mockResolvedValueOnce({
        items: [{ code: 'test', message: 'Запись восстановлена', source_location: null }],
        total: 1,
        next_offset: null,
      }),
  } as unknown as NativeBridge;
  render(DiagnosticsList, { props: { id: 'job', count: 1, bridge: native, expanded: true } });
  expect(await screen.findByRole('alert')).toHaveTextContent('Не удалось прочитать журнал');
  await fireEvent.click(screen.getByRole('button', { name: 'Повторить чтение' }));
  expect(await screen.findByText('Запись восстановлена')).toBeVisible();
  expect(screen.queryByText('Не удалось прочитать журнал')).not.toBeInTheDocument();
  expect(native.readDiagnostics).toHaveBeenLastCalledWith('job', 0, 50);
});

it('запоздалый ответ предыдущего задания не заменяет открытый журнал', async () => {
  let resolveOld!: (value: DiagnosticPage) => void;
  const native = {
    readDiagnostics: vi
      .fn()
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveOld = resolve;
          }),
      )
      .mockResolvedValueOnce({
        items: [{ code: 'test', message: 'Новое задание', source_location: null }],
        total: 1,
        next_offset: null,
      }),
  } as unknown as NativeBridge;
  const view = render(DiagnosticsList, {
    props: { id: 'old', count: 1, bridge: native, expanded: true },
  });
  await view.rerender({ id: 'new' });
  expect(await screen.findByText('Новое задание')).toBeVisible();
  resolveOld({
    items: [{ code: 'test', message: 'Старое задание', source_location: null }],
    total: 1,
    next_offset: null,
  });
  await Promise.resolve();
  expect(screen.queryByText('Старое задание')).not.toBeInTheDocument();
  expect(screen.getByText('Новое задание')).toBeVisible();
});
