import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/vue';
import App from '../../src/App.vue';
import fixtures from './fixtures/contracts.json';
import type { JobSnapshot, NativeBridge } from '../../src/shared/contracts';
describe('Повторная конвертация', () => {
  it('перезапускает готовое задание с прежними идентификаторами источников', async () => {
    const job = fixtures.succeeded as unknown as JobSnapshot;
    const startJob = vi.fn(async () => ({ ...job, generation: 3, sequence: 1, state: 'queued' }));
    const bridge = {
      getJobs: async () => [job],
      onJobEvent: async () => () => {},
      startJob,
    } as unknown as NativeBridge;
    render(App, { props: { bridge } });
    await fireEvent.click(await screen.findByRole('button', { name: 'Повторить конвертацию' }));
    await waitFor(() => expect(startJob).toHaveBeenCalledWith(job.id));
  });
});
