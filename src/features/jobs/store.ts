import { reactive, ref } from 'vue';
import type { JobOptionsUpdate, JobSnapshot, NativeBridge } from '../../shared/contracts';
import { errorMessage } from '../../shared/format';

export function createJobStore(bridge: NativeBridge) {
  const jobs = reactive<Record<string, JobSnapshot>>({});
  const error = ref('');
  const pending = reactive<Record<string, boolean>>({});
  const deleted = new Set<string>();
  const resolvedOffers = reactive(new Set<string>());
  let disposed = false;
  let unsubscribe: (() => void) | undefined;
  function mergeSnapshot(snapshot: JobSnapshot) {
    if (disposed || deleted.has(snapshot.id)) return;
    const old = jobs[snapshot.id];
    if (
      old &&
      (snapshot.generation < old.generation ||
        (snapshot.generation === old.generation && snapshot.sequence <= old.sequence))
    )
      return;
    jobs[snapshot.id] = snapshot;
  }
  const ready = (async () => {
    try {
      unsubscribe = await bridge.onJobEvent((event) => mergeSnapshot(event.snapshot));
      if (disposed) {
        unsubscribe();
        return;
      }
      const initial = await bridge.getJobs();
      initial.forEach(mergeSnapshot);
    } catch (e) {
      if (!disposed) error.value = errorMessage(e);
    }
  })();
  async function action(id: string, run: () => Promise<JobSnapshot | void>) {
    if (pending[id]) return false;
    pending[id] = true;
    error.value = '';
    try {
      const snapshot = await run();
      if (snapshot) mergeSnapshot(snapshot);
      return true;
    } catch (e) {
      error.value = errorMessage(e);
      return false;
    } finally {
      pending[id] = false;
    }
  }
  return {
    jobs,
    error,
    pending,
    resolvedOffers,
    ready,
    mergeSnapshot,
    startJob: (id: string) => action(id, () => bridge.startJob(id)),
    pauseDownload: (id: string) => action(id, () => bridge.pauseDownload(id)),
    resumeDownload: (id: string) => action(id, () => bridge.resumeDownload(id)),
    cancelJob: (id: string) => action(id, () => bridge.cancelJob(id)),
    deleteJob: (id: string) =>
      action(id, async () => {
        await bridge.deleteJob(id);
        deleted.add(id);
        delete jobs[id];
      }),
    updateJob: (id: string, update: JobOptionsUpdate) =>
      action(id, () => bridge.updateJob(id, update)),
    resolveAdditional: (id: string, offerId: string, convert: boolean) =>
      action(`offer:${offerId}`, async () => {
        const children = await bridge.resolveAdditionalDictionaries(id, offerId, convert);
        children.forEach(mergeSnapshot);
        resolvedOffers.add(offerId);
      }),
    exportResult: (id: string) => action(id, () => bridge.exportResult(id)),
    openResult: (id: string) => action(id, () => bridge.openResult(id)),
    dispose() {
      disposed = true;
      unsubscribe?.();
    },
  };
}
