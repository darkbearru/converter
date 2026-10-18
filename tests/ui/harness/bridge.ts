import type {
  DiagnosticPage,
  JobDraft,
  JobEvent,
  JobOptionsUpdate,
  JobSnapshot,
  NativeBridge,
  SourcePurpose,
} from '../../../src/shared/contracts';
import fixtures from '../fixtures/contracts.json';

const samples = fixtures as unknown as Record<string, JobSnapshot>;
const copy = <T>(value: T): T => structuredClone(value);
export function createHarness(scenario: string | null) {
  const jobs = new Map<string, JobSnapshot>();
  const listeners = new Set<(event: JobEvent) => void>();
  let currentId = samples.local.id;
  let nextSequence = 110;
  const emit = (snapshot: JobSnapshot) => {
    jobs.set(snapshot.id, copy(snapshot));
    listeners.forEach((listener) => listener({ snapshot: copy(snapshot) }));
    return copy(snapshot);
  };
  function next(sample: string) {
    const job = copy(samples[sample]);
    job.id = currentId;
    job.sequence = ++nextSequence;
    return emit(job);
  }
  if (scenario && samples[scenario]) jobs.set(currentId, copy(samples[scenario]));
  if (scenario === 'long') {
    const job = copy(samples.succeeded);
    job.display_name = `Большой русско-английский энциклопедический словарь ${'очень_длинное_название_'.repeat(16)}.dsl`;
    job.diagnostic_count = 70;
    jobs.set(currentId, job);
  }
  if (scenario === 'awaiting') {
    const job = copy(samples.local);
    job.state = 'awaiting_input';
    job.error = { code: 'encoding_required', message: 'Выберите кодировку и словарь в архиве' };
    job.input_choices = [
      { id: 'one', display_name: 'Первый.dsl' },
      { id: 'two', display_name: 'Второй.dsl' },
    ];
    jobs.set(currentId, job);
  }
  const bridge: NativeBridge = {
    async listFormats() {
      return [
        { id: 'dsl', name: 'Lingvo DSL', extensions: ['dsl', 'dsl.dz'], supports_resources: true },
      ];
    },
    async pickSources(purpose: SourcePurpose) {
      return [
        {
          import_id: purpose === 'dictionaries' ? 'main' : 'resource',
          display_name: purpose === 'dictionaries' ? 'Словарь.dsl' : 'Ресурсы.zip',
          bytes: 1024,
          is_directory: purpose === 'resource_directory',
        },
      ];
    },
    async pickOutput() {
      return { desktop_folder: '/selected/output' };
    },
    async createJob(draft: JobDraft) {
      const snapshot = copy(samples[draft.source.kind === 'remote' ? 'downloading' : 'succeeded']);
      snapshot.id = currentId;
      snapshot.sequence = ++nextSequence;
      if (draft.source.kind === 'local') {
        snapshot.download = copy(samples.local.download);
        snapshot.display_name = 'Словарь.dsl';
      }
      return emit(snapshot);
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
    async startJob(id) {
      currentId = id;
      return next('succeeded');
    },
    async pauseDownload(id) {
      currentId = id;
      return next('paused');
    },
    async resumeDownload(id) {
      currentId = id;
      return next('downloading');
    },
    async cancelJob(id) {
      const job = copy(jobs.get(id)!);
      job.state = 'cancelled';
      job.sequence = ++nextSequence;
      job.conversion.status = 'cancelled';
      return emit(job);
    },
    async updateJob(id, update: JobOptionsUpdate) {
      const job = copy(jobs.get(id)!);
      job.state = 'queued';
      job.error = null;
      job.options = update.options ?? job.options;
      job.sequence = ++nextSequence;
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
      job.sequence = ++nextSequence;
      job.export = {
        status: 'succeeded',
        saved_files: ['dictionary.mdx', 'dictionary.mdd', 'conversion-report.json'],
        error: null,
      };
      return emit(job);
    },
    async openResult() {},
    async readDiagnostics(_id, offset, limit): Promise<DiagnosticPage> {
      const end = Math.min(70, offset + limit);
      return {
        items: Array.from({ length: Math.max(0, end - offset) }, (_, index) => ({
          code: 'test_warning',
          message: `Предупреждение ${offset + index + 1}${offset + index === 0 ? ': <img src=x onerror=alert(1)>' : ''}`,
          source_location: { source: 'Словарь.dsl', line: offset + index + 10 },
        })),
        next_offset: end < 70 ? end : null,
        total: 70,
      };
    },
  };
  return {
    bridge,
    advance: () => next('converting'),
    finish: () => next('succeeded'),
    stale: () => {
      listeners.forEach((listener) => listener({ snapshot: copy(samples.stale) }));
    },
  };
}
