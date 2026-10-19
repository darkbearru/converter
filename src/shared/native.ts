import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { JobEvent, NativeBridge, SourceDropEvent } from './contracts';

function unavailable(): never {
  throw {
    code: 'native_unavailable',
    message: 'Open the Converter desktop application to work with files.',
    message_key: 'app.native_required',
  };
}
async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) unavailable();
  return invoke<T>(command, args);
}
export const nativeBridge: NativeBridge = {
  getAppPreferences: () => call('get_app_preferences'),
  setAppLanguage: (language) => call('set_app_language', { language }),
  getAppVersion: () => call('get_app_version'),
  checkForUpdates: () => call('check_for_updates'),
  openReleasePage: () => call('open_release_page'),
  async onAboutRequested(callback) {
    if (!isTauri()) unavailable();
    return listen('app:about', callback);
  },
  readClipboardText: () => call('read_clipboard_text'),
  openSupportPage: () => call('open_support_page'),
  openIssue: (id) => call('open_issue', { id: id ?? null }),
  listFormats: () => call('list_formats'),
  pickSources: (purpose) => call('pick_sources', { purpose }),
  pickOutput: () => call('pick_output'),
  createJob: (draft) => call('create_job', { draft }),
  startJob: (id) => call('start_job', { id }),
  provideArchivePassword: (id, archiveId, password) =>
    call('provide_archive_password', { id, archiveId, password }),
  pauseDownload: (id) => call('pause_download', { id }),
  resumeDownload: (id) => call('resume_download', { id }),
  cancelJob: (id) => call('cancel_job', { id }),
  deleteJob: (id) => call('delete_job', { id }),
  updateJob: (id, update) => call('update_job', { id, update }),
  resolveAdditionalDictionaries: (id, offerId, convert) =>
    call('resolve_additional_dictionaries', { jobId: id, offerId, convert }),
  getJobs: () => call('get_jobs'),
  readDiagnostics: (id, offset, limit) => call('read_diagnostics', { id, offset, limit }),
  exportResult: (id) => call('export_result', { id }),
  openResult: (id) => call('open_result', { id }),
  getResultPath: (id) => call('get_result_path', { id }),
  async onSourceDrop(callback) {
    if (!isTauri()) unavailable();
    return listen<SourceDropEvent>('source:drop', (event) => callback(event.payload));
  },
  async onJobEvent(callback) {
    if (!isTauri()) unavailable();
    return listen<JobEvent>('job:event', (event) => callback(event.payload));
  },
};
