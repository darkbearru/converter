import type {
  DiagnosticPage,
  JobDraft,
  JobEvent,
  JobSnapshot,
  NativeBridge,
} from '../../../src/shared/contracts';
import fixtures from '../fixtures/contracts.json';
const samples = fixtures as unknown as Record<string, JobSnapshot>;
// Как Tauri IPC: передаём JSON, в том числе значения из реактивных форм Vue.
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
export function createHarness(scenario: string | null) {
  const jobs = new Map<string, JobSnapshot>();
  const listeners = new Set<(event: JobEvent) => void>();
  let currentId = samples.local.id;
  let nextSequence = 110;
  let nextId = 0;
  const emit = (snapshot: JobSnapshot) => {
    snapshot.sequence = ++nextSequence;
    jobs.set(snapshot.id, copy(snapshot));
    listeners.forEach((listener) => listener({ snapshot: copy(snapshot) }));
    return copy(snapshot);
  };
  function next(sample: string) {
    const old = jobs.get(currentId);
    const job = copy(samples[sample]);
    job.id = currentId;
    job.display_name = old?.display_name ?? 'main.dsl';
    job.generation = old?.generation ?? job.generation;
    job.source_bytes = old?.source_bytes ?? 25 * 1024 ** 2;
    job.prepare_only = false;
    return emit(job);
  }
  if (scenario && samples[scenario]) {
    const job = copy(samples[scenario]);
    job.display_name = 'main.dsl';
    if (['downloading', 'paused', 'unknown'].includes(scenario)) job.prepare_only = true;
    jobs.set(currentId, job);
  }
  if (scenario === 'ready' || scenario === 'ready-resources') {
    const job = copy(samples.local);
    job.state = 'ready';
    job.prepare_only = true;
    job.display_name = 'main.dsl';
    job.source_bytes = 25 * 1024 ** 2;
    if (scenario === 'ready-resources')
      job.resource_attachments = [
        { source_id: 'cached-resource', display_name: 'images-and-audio.zip', bytes: 1024 * 1024 },
      ];
    jobs.set(currentId, job);
  }
  if (scenario === 'stack') {
    for (let i = 0; i < 2; i++) {
      const job = copy(samples.converting);
      job.id = `stack-${i}`;
      job.display_name = i ? 'main.dsl' : 'english.dsl';
      jobs.set(job.id, job);
      currentId = job.id;
    }
  }
  if (['history-one', 'history-many', 'history-active'].includes(scenario ?? '')) {
    const count = scenario === 'history-one' ? 1 : 12;
    for (let index = 0; index < count; index++) {
      const job = copy(samples.succeeded);
      job.id = `history-${index}`;
      job.display_name = `Словарь ${index + 1}.dsl`;
      jobs.set(job.id, job);
    }
    if (scenario === 'history-active') {
      for (let index = 0; index < 2; index++) {
        const job = copy(samples.converting);
        job.id = `active-${index}`;
        job.display_name = `В работе ${index + 1}.dsl`;
        jobs.set(job.id, job);
      }
    }
  }
  if (scenario === 'long') {
    const job = copy(samples.succeeded);
    job.display_name = `Большой русско-английский словарь ${'очень_длинное_название_'.repeat(16)}.dsl`;
    job.diagnostic_count = 70;
    job.had_errors = true;
    jobs.set(currentId, job);
  }
  if (scenario === 'awaiting') {
    const job = copy(samples.local);
    job.state = 'awaiting_input';
    job.had_errors = true;
    job.error = { code: 'encoding_required', message: 'Выберите кодировку и словарь в архиве' };
    job.input_choices = [
      { id: 'one', display_name: 'Первый.dsl' },
      { id: 'two', display_name: 'Второй.dsl' },
    ];
    jobs.set(currentId, job);
  }
  if (scenario === 'resource-failed') {
    const job = copy(samples.downloading);
    job.state = 'download_failed';
    job.prepare_only = false;
    job.display_name = 'main.dsl';
    job.download.files[0]!.state = 'failed';
    jobs.set(currentId, job);
  }
  const bridge: NativeBridge = {
    async listFormats() {
      return [
        { id: 'dsl', name: 'Lingvo DSL', extensions: ['dsl', 'dsl.dz'], supports_resources: true },
      ];
    },
    async pickSources(purpose) {
      return [
        {
          import_id: purpose === 'dictionaries' ? 'main' : 'resource',
          display_name: purpose === 'dictionaries' ? 'main.dsl' : 'Ресурсы.zip',
          bytes: 25 * 1024 ** 2,
          is_directory: purpose === 'resource_directory',
        },
      ];
    },
    async pickOutput() {
      return { desktop_folder: '/selected/output' };
    },
    async createJob(draft: JobDraft) {
      currentId = `created-${++nextId}`;
      const job = copy(samples[draft.source.kind === 'remote' ? 'downloading' : 'converting']);
      job.id = currentId;
      job.display_name = 'main.dsl';
      job.prepare_only = draft.prepare_only ?? false;
      job.source_bytes = 25 * 1024 ** 2;
      job.options = draft.options;
      job.output = draft.output;
      job.metadata = draft.metadata_override;
      if (draft.source.kind === 'local') job.download = copy(samples.local.download);
      return emit(job);
    },
    async getJobs() {
      return [...jobs.values()].map(copy);
    },
    async onJobEvent(callback) {
      listeners.add(callback);
      return () => {
        listeners.delete(callback);
      };
    },
    async onSourceDrop() {
      return () => {};
    },
    async startJob(id) {
      currentId = id;
      return next('converting');
    },
    async pauseDownload(id) {
      currentId = id;
      const job = copy(jobs.get(id)!);
      job.state = 'download_paused';
      for (const file of job.download.files) {
        file.state = 'paused';
        file.bytes_per_second = null;
      }
      return emit(job);
    },
    async resumeDownload(id) {
      currentId = id;
      const job = copy(jobs.get(id)!);
      job.state = 'downloading';
      for (const file of job.download.files) file.state = 'downloading';
      return emit(job);
    },
    async cancelJob(id) {
      const job = copy(jobs.get(id)!);
      job.state = 'cancelled';
      job.conversion.status = 'cancelled';
      for (const file of job.download.files)
        if (file.state !== 'completed') {
          file.state = 'cancelled';
          file.bytes_per_second = null;
        }
      return emit(job);
    },
    async updateJob(id, update) {
      const job = copy(jobs.get(id)!);
      if (job.state !== 'ready') job.state = 'queued';
      job.error = null;
      job.options = update.options ?? job.options;
      job.output = update.output ?? job.output;
      if (update.dictionary_id === '*') {
        const child = copy(job);
        child.id = 'child';
        child.display_name = 'Второй.dsl';
        emit(child);
      }
      return emit(job);
    },
    async exportResult(id) {
      const job = copy(jobs.get(id)!);
      job.export = {
        status: 'succeeded',
        saved_files: ['dictionary.mdx', 'dictionary.mdd', 'conversion-report.json'],
        error: null,
      };
      return emit(job);
    },
    async openResult() {},
    async getResultPath() {
      return '/Словари/Англо-русский-converted';
    },
    async readDiagnostics(id, offset, limit): Promise<DiagnosticPage> {
      const total = jobs.get(id)?.diagnostic_count ?? 0;
      const end = Math.min(total, offset + limit);
      return {
        items: Array.from({ length: Math.max(0, end - offset) }, (_, index) => ({
          code: 'test_warning',
          message: `Предупреждение ${offset + index + 1}${offset + index === 0 ? ': <img src=x onerror=alert(1)>' : ''}`,
          source_location: { source: 'Словарь.dsl', line: offset + index + 10 },
        })),
        next_offset: end < total ? end : null,
        total,
      };
    },
  };
  return {
    bridge,
    advance: () => {
      const job = jobs.get(currentId);
      if (job?.prepare_only) {
        const ready = copy(job);
        ready.state = 'ready';
        ready.display_name = 'main.dsl';
        ready.source_bytes = 25 * 1024 ** 2;
        ready.download.percent = 100;
        ready.download.files.forEach((f) => {
          f.state = 'completed';
          f.received_bytes = f.total_bytes ?? f.received_bytes;
          f.confirmed_bytes = f.received_bytes;
          f.bytes_per_second = null;
        });
        ready.download.received_bytes = ready.download.files.reduce(
          (sum, file) => sum + file.received_bytes,
          0,
        );
        ready.download.confirmed_bytes = ready.download.received_bytes;
        ready.download.total_bytes = ready.download.received_bytes;
        return emit(ready);
      }
      return next('converting');
    },
    finish: () => next('succeeded'),
    stale: () =>
      listeners.forEach((listener) =>
        listener({ snapshot: { ...copy(samples.stale), id: currentId } }),
      ),
  };
}
