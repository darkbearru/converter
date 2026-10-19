import { preferencesFor, type LanguagePreference } from '../../../src/shared/i18n/preferences';
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
  const aboutListeners = new Set<() => void>();
  let updateOffline = scenario === 'update-offline';
  let checkCount = 0;
  let releaseOpened = false;
  let currentId = samples.local.id;
  let nextSequence = 110;
  let nextId = 0;
  let finishAdditional: (() => void) | undefined;
  let additionalFailed = false;
  let additionalHandled = false;
  const additionalOffer = {
    id: 'offer-main',
    main_name: 'Universal (Ch-Ru)',
    dictionaries: [{ id: 'phrase', display_name: 'PhraseBookRuCh.dsl.dz' }],
  };
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
  if (scenario === 'preview' || scenario === 'localized-preview') {
    for (const [index, name] of (scenario === 'localized-preview'
      ? ['English dictionary.dsl', 'Definitions.dsl']
      : ['Англо-русский.dsl', 'Толковый.dsl']
    ).entries()) {
      const job = copy(samples.succeeded);
      job.id = `preview-${index}`;
      job.display_name = name;
      if (index === 1) {
        job.state = 'succeeded_with_warnings';
        job.diagnostic_count = 1;
        job.result!.summary.warnings = 1;
      }
      jobs.set(job.id, job);
    }
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
  if (scenario === 'warning') {
    const job = copy(samples.succeeded);
    job.state = 'succeeded_with_warnings';
    job.display_name = 'PhoneticListChCh.dsl.dz';
    job.diagnostic_count = 28173;
    job.result!.summary.warnings = 28173;
    jobs.set(currentId, job);
  }
  if (scenario === 'failed') {
    const job = copy(samples.succeeded);
    job.state = 'failed';
    job.display_name = 'Dictionary.dsl';
    job.result = null;
    job.error = { code: 'invalid_markup', message: 'Нарушена структура статьи: не закрыта ссылка' };
    job.diagnostic_count = 1;
    jobs.set(currentId, job);
  }
  if (scenario === 'grouped-warnings') {
    const job = copy(samples.succeeded);
    job.state = 'succeeded_with_warnings';
    job.display_name = 'Dictionary.dsl';
    job.diagnostic_count = 2;
    job.result!.summary.warnings = 2;
    jobs.set(currentId, job);
  }
  if (scenario?.startsWith('additional')) {
    const job = copy(samples.converting);
    job.display_name = 'UniversalChRu.dsl.dz';
    job.additional_dictionaries = copy(additionalOffer);
    if (scenario === 'additional-long') {
      job.additional_dictionaries.dictionaries[0]!.display_name = `${'Очень_длинное_имя_'.repeat(20)}.dsl`;
    }
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
  if (scenario === 'archive-password') {
    const job = copy(samples.local);
    job.state = 'awaiting_input';
    job.display_name = 'locked.zip';
    job.error = {
      code: 'archive_password_required',
      message: 'Для ZIP требуется пароль',
      message_key: 'messages.archive_password_required',
      message_args: { archive_id: 'archive-main', name: 'locked.zip' },
    };
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
  let language: LanguagePreference = 'system';
  const bridge: NativeBridge = {
    async provideArchivePassword(id, archiveId, password) {
      const job = copy(jobs.get(id)!);
      if (archiveId !== 'archive-main') throw new Error('Unexpected archive');
      job.generation++;
      if (password === 'secret') {
        job.state = 'converting';
        job.error = null;
      } else {
        job.state = 'awaiting_input';
        job.error = {
          code: 'archive_password_invalid',
          message: 'Пароль ZIP не подошёл',
          message_key: 'messages.archive_password_invalid',
          message_args: { name: 'locked.zip', archive_id: 'archive-main' },
        };
      }
      return emit(job);
    },
    async getAppPreferences() {
      return preferencesFor(language, 'ru-RU');
    },
    async setAppLanguage(next) {
      language = next;
      return preferencesFor(language, 'ru-RU');
    },
    async getAppVersion() {
      return '0.2.3';
    },
    async checkForUpdates() {
      checkCount++;
      if (updateOffline) throw new Error('Не удалось проверить обновления');
      return { version: scenario === 'update-available' ? '0.3.0' : null };
    },
    async downloadUpdate(_version) {
      releaseOpened = true;
    },
    async onAboutRequested(callback) {
      aboutListeners.add(callback);
      return () => {
        aboutListeners.delete(callback);
      };
    },
    async readClipboardText() {
      return 'https://example.org/pasted.dsl';
    },
    async openSupportPage() {},
    async openIssue() {},
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
    async resolveAdditionalDictionaries(id, offerId, convert) {
      if (offerId !== additionalOffer.id) throw new Error('Предложение устарело');
      if (scenario === 'additional-error' && !additionalFailed) {
        additionalFailed = true;
        throw new Error('Не удалось сохранить очередь. Повторите попытку.');
      }
      if (scenario === 'additional-busy')
        await new Promise<void>((resolve) => {
          finishAdditional = resolve;
        });
      const parent = copy(jobs.get(id)!);
      parent.additional_dictionaries = null;
      emit(parent);
      if (additionalHandled)
        return [...jobs.values()].filter((job) => job.id.startsWith('independent-')).map(copy);
      additionalHandled = true;
      if (!convert) return [];
      const child = copy(samples.converting);
      child.id = `independent-${++nextId}`;
      child.display_name = 'PhraseBookRuCh.dsl.dz';
      child.additional_dictionaries = null;
      return [emit(child)];
    },
    async deleteJob(id) {
      jobs.delete(id);
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
          ...(scenario === 'localized-preview'
            ? { message_key: 'messages.missing_reference', message_args: { target: 'sample' } }
            : {}),
          severity: 'warning',
          occurrences: scenario === 'grouped-warnings' && offset + index === 0 ? 1234 : 1,
          message:
            scenario === 'grouped-warnings'
              ? offset + index === 0
                ? 'Отсутствует ресурс: voice.wav'
                : 'Не найдено сокращение: rare.'
              : scenario === 'preview'
                ? 'Не найдена статья для ссылки: пример'
                : scenario === 'warning'
                  ? 'Не найдена статья для ссылки: 儿'
                  : `Предупреждение ${offset + index + 1}${offset + index === 0 ? ': <img src=x onerror=alert(1)>' : ''}`,
          source_location: {
            source: scenario === 'localized-preview' ? 'dictionary.dsl' : 'Словарь.dsl',
            line: offset + index + 10,
          },
        })),
        next_offset: end < total ? end : null,
        total,
      };
    },
  };
  return {
    bridge,
    openAbout: () => aboutListeners.forEach((callback) => callback()),
    openedRelease: () => releaseOpened,
    updateChecks: () => checkCount,
    restoreUpdates: () => {
      updateOffline = false;
    },
    advance: () => {
      if (finishAdditional) {
        const finish = finishAdditional;
        finishAdditional = undefined;
        finish();
        return;
      }
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
    stale: () => {
      if (scenario?.startsWith('additional') && additionalHandled) {
        const parent = copy(jobs.get(currentId)!);
        parent.additional_dictionaries = copy(additionalOffer);
        emit(parent);
        return;
      }
      listeners.forEach((listener) =>
        listener({ snapshot: { ...copy(samples.stale), id: currentId } }),
      );
    },
  };
}
