import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/vue';
import App from '../../src/App.vue';
import fixtures from './fixtures/contracts.json';
import type { JobSnapshot, NativeBridge } from '../../src/shared/contracts';
describe('Повторная конвертация', () => {
  it('запускает уточнённое задание даже после закрытия диалога во время сохранения', async () => {
    const job = {
      ...structuredClone(fixtures.local),
      state: 'awaiting_input',
    } as unknown as JobSnapshot;
    let resolveUpdate!: (job: JobSnapshot) => void;
    const update = new Promise<JobSnapshot>((resolve) => {
      resolveUpdate = resolve;
    });
    const startJob = vi.fn(async () => ({ ...job, state: 'queued' }));
    const bridge = {
      getJobs: async () => [job],
      onJobEvent: async () => () => {},
      updateJob: vi.fn(() => update),
      startJob,
    } as unknown as NativeBridge;
    render(App, { props: { bridge } });
    await fireEvent.click(
      await screen.findByRole('button', { name: new RegExp(job.display_name) }),
    );
    await fireEvent.click(screen.getByRole('button', { name: 'Уточнить параметры' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Сохранить параметры и запустить' }));
    await waitFor(() => expect(bridge.updateJob).toHaveBeenCalled());
    await fireEvent.click(screen.getByRole('button', { name: 'Закрыть' }));
    resolveUpdate({ ...job, state: 'queued', sequence: job.sequence + 1 });
    await waitFor(() => expect(startJob).toHaveBeenCalledWith(job.id));
  });
  it('перезапускает готовое задание с прежними идентификаторами источников', async () => {
    const job = fixtures.succeeded as unknown as JobSnapshot;
    const startJob = vi.fn(async () => ({ ...job, generation: 3, sequence: 1, state: 'queued' }));
    const bridge = {
      getJobs: async () => [job],
      onJobEvent: async () => () => {},
      startJob,
      getResultPath: async () => '/result',
    } as unknown as NativeBridge;
    render(App, { props: { bridge } });
    await fireEvent.click(
      await screen.findByRole('button', { name: new RegExp(job.display_name) }),
    );
    await fireEvent.click(await screen.findByRole('button', { name: 'Повторить' }));
    await waitFor(() => expect(startJob).toHaveBeenCalledWith(job.id));
  });
});

it('удаляет запись из истории только после подтверждения', async () => {
  const job = fixtures.succeeded as unknown as JobSnapshot;
  const bridge = {
    getJobs: async () => [job],
    onJobEvent: async () => () => {},
    getResultPath: async () => '/result',
    deleteJob: vi.fn(async () => {}),
  } as unknown as NativeBridge;
  render(App, { props: { bridge } });
  await fireEvent.click(await screen.findByRole('button', { name: new RegExp(job.display_name) }));
  await fireEvent.click(screen.getByRole('button', { name: 'Удалить' }));
  expect(screen.getByRole('dialog')).toHaveTextContent('Вы действительно хотите удалить');
  await fireEvent.click(screen.getByRole('button', { name: 'Отмена' }));
  expect(screen.getByText(job.display_name)).toBeVisible();
  expect(bridge.deleteJob).not.toHaveBeenCalled();
  await fireEvent.click(screen.getByRole('button', { name: 'Удалить' }));
  await fireEvent.click(
    within(screen.getByRole('dialog')).getByRole('button', { name: 'Удалить' }),
  );
  await waitFor(() => expect(screen.queryByText(job.display_name)).not.toBeInTheDocument());
  await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
});

it('Escape не закрывает подтверждение во время удаления', async () => {
  const job = fixtures.succeeded as unknown as JobSnapshot;
  let finish!: () => void;
  render(App, {
    global: { stubs: { transition: false } },
    props: {
      bridge: {
        getJobs: async () => [job],
        onJobEvent: async () => () => {},
        getResultPath: async () => '/result',
        deleteJob: () =>
          new Promise<void>((resolve) => {
            finish = resolve;
          }),
      } as unknown as NativeBridge,
    },
  });
  await fireEvent.click(await screen.findByRole('button', { name: new RegExp(job.display_name) }));
  await fireEvent.click(screen.getByRole('button', { name: 'Удалить' }));
  await new Promise<void>((resolve) =>
    requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
  );
  await fireEvent.click(
    within(screen.getByRole('dialog')).getByRole('button', { name: 'Удалить' }),
  );
  await fireEvent.keyDown(document, { key: 'Escape', code: 'Escape' });
  await new Promise((resolve) => setTimeout(resolve, 50));
  expect(screen.getByRole('dialog')).toBeVisible();
  finish();
  await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
});
