import { describe, expect, it, vi } from 'vitest';
import { createJobStore } from '../../src/features/jobs/store';
import type { JobEvent, JobSnapshot, NativeBridge } from '../../src/shared/contracts';
import fixtures from './fixtures/contracts.json';

const jobs = fixtures as unknown as Record<string, JobSnapshot>;
describe('Очередь событий Rust', () => {
  it('подписывается до снимка и отбрасывает старые generation и sequence', async () => {
    const order: string[] = [];
    let listener!: (event: JobEvent) => void;
    const unsubscribe = vi.fn();
    const bridge = {
      onJobEvent: async (callback: typeof listener) => {
        order.push('subscribe');
        listener = callback;
        return unsubscribe;
      },
      getJobs: async () => {
        order.push('list');
        listener({ snapshot: jobs.succeeded });
        return [jobs.downloading];
      },
    } as unknown as NativeBridge;
    const store = createJobStore(bridge);
    await store.ready;
    listener({ snapshot: jobs.stale });
    listener({ snapshot: jobs.converting });
    expect(order).toEqual(['subscribe', 'list']);
    expect(store.jobs[jobs.succeeded.id].state).toBe('succeeded');
    store.dispose();
    expect(unsubscribe).toHaveBeenCalledOnce();
  });
  it('снимает запоздавшую подписку после закрытия', async () => {
    let resolve!: (fn: () => void) => void;
    const unsubscribe = vi.fn();
    const getJobs = vi.fn(async () => []);
    const bridge = {
      onJobEvent: () =>
        new Promise((r) => {
          resolve = r;
        }),
      getJobs,
    } as unknown as NativeBridge;
    const store = createJobStore(bridge);
    store.dispose();
    resolve(unsubscribe);
    await store.ready;
    expect(unsubscribe).toHaveBeenCalledOnce();
    expect(getJobs).not.toHaveBeenCalled();
  });
  it('ошибка действия не уничтожает готовый результат', async () => {
    const bridge = {
      onJobEvent: async () => () => {},
      getJobs: async () => [jobs.succeeded],
      exportResult: async () => {
        throw { code: 'output_unavailable', message: 'Папка недоступна' };
      },
    } as unknown as NativeBridge;
    const store = createJobStore(bridge);
    await store.ready;
    await store.exportResult(jobs.succeeded.id);
    expect(store.error.value).toBe('Папка недоступна');
    expect(store.jobs[jobs.succeeded.id].result).not.toBeNull();
    store.dispose();
  });
});
