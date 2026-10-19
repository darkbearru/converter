import type { AppPreferences, LanguagePreference } from './i18n/preferences';
export interface LocalizedMessage {
  message: string;
  message_key?: string | null;
  message_args?: Record<string, string | number>;
}
export interface AppError extends LocalizedMessage {
  code: string;
  message: string;
}
export type Encoding = 'auto' | { manual: string };
export interface ReadOptions {
  encoding: Encoding;
}
export interface DictionaryMetadata {
  title: string;
  description: string;
  index_language: string;
  contents_language: string;
  source_encoding: string;
}
export type SourceInput =
  | { kind: 'local'; import_id: string }
  | { kind: 'remote'; url: string; expected_sha256: string | null };
export type OutputChoice = 'app_library' | { desktop_folder: string };
export interface JobDraft {
  source: SourceInput;
  resources: SourceInput[];
  output: OutputChoice;
  options: ReadOptions;
  metadata_override: DictionaryMetadata | null;
  auto_start: boolean;
  prepare_only?: boolean;
}
export interface JobOptionsUpdate {
  options?: ReadOptions;
  metadata_override?: DictionaryMetadata;
  output?: OutputChoice;
  dictionary_id?: string;
  resources?: string[];
  resource_sources?: SourceInput[];
  retained_resource_ids?: string[];
}
export interface ResourceAttachment {
  source_id: string;
  display_name: string;
  bytes: number | null;
}
export type JobState =
  | 'queued'
  | 'ready'
  | 'downloading'
  | 'download_retry_wait'
  | 'download_paused'
  | 'download_failed'
  | 'importing'
  | 'inspecting'
  | 'awaiting_input'
  | 'converting'
  | 'validating'
  | 'publishing'
  | 'succeeded'
  | 'succeeded_with_warnings'
  | 'failed'
  | 'cancelled'
  | 'interrupted';
export type TransferState =
  | 'pending'
  | 'downloading'
  | 'retry_wait'
  | 'paused'
  | 'verifying'
  | 'completed'
  | 'failed'
  | 'cancelled';
export interface DownloadFileProgress {
  source_id: string;
  state: TransferState;
  received_bytes: number;
  confirmed_bytes: number;
  total_bytes: number | null;
  bytes_per_second: number | null;
  restart_reason: string | null;
  restart_message?: LocalizedMessage | null;
}
export interface DownloadJobProgress {
  kind: 'local' | 'remote';
  files: DownloadFileProgress[];
  received_bytes: number;
  confirmed_bytes: number;
  total_bytes: number | null;
  percent: number | null;
}
export type Stage =
  'inspect' | 'read' | 'sort' | 'write_mdx' | 'write_mdd' | 'validate' | 'publish';
export interface StageProgress {
  stage: Stage;
  completed: number;
  total: number | null;
  unit: 'bytes' | 'entries' | 'resources';
  articles: number;
  resources: number;
}
export interface ConversionJobProgress {
  status: 'not_started' | 'running' | 'succeeded' | 'failed' | 'cancelled' | 'interrupted';
  current: StageProgress | null;
  completed_stages: StageProgress[];
}
export interface ConversionSummary {
  source_articles: number;
  output_keys: number;
  resources: number;
  warnings: number;
  stages: StageProgress[];
  sort_metrics: { merge_passes: number; peak_sort_bytes: number };
}
export interface ResultInfo {
  id: string;
  files: { name: string; bytes: number }[];
  summary: ConversionSummary;
  can_open: boolean;
  can_export: boolean;
}
export interface ExportSnapshot {
  status: 'not_started' | 'running' | 'succeeded' | 'failed' | 'cancelled';
  saved_files: string[];
  error: AppError | null;
}
export interface AdditionalDictionariesOffer {
  id: string;
  main_name: string;
  dictionaries: { id: string; display_name: string }[];
}
export interface JobSnapshot {
  additional_dictionaries?: AdditionalDictionariesOffer | null;
  resource_attachments?: ResourceAttachment[];
  id: string;
  display_name: string;
  source_bytes?: number | null;
  prepare_only?: boolean;
  had_errors?: boolean;
  output?: OutputChoice;
  generation: number;
  run_id: string;
  sequence: number;
  state: JobState;
  download: DownloadJobProgress;
  conversion: ConversionJobProgress;
  result: ResultInfo | null;
  export: ExportSnapshot;
  error: AppError | null;
  input_choices: { id: string; display_name: string }[];
  diagnostic_count: number;
  options: ReadOptions;
  metadata: DictionaryMetadata | null;
}
export interface JobEvent {
  snapshot: JobSnapshot;
}
export interface ImportedSourceInfo {
  import_id: string;
  display_name: string;
  bytes: number | null;
  is_directory: boolean;
}
export interface FormatDescriptor {
  id: string;
  name: string;
  extensions: string[];
  supports_resources: boolean;
}
export interface Diagnostic extends LocalizedMessage {
  code: string;
  message: string;
  source_location: { source: string; line: number } | null;
  // В сохранённых журналах ранних версий этих полей нет.
  severity?: 'info' | 'warning';
  occurrences?: number;
}
export interface DiagnosticPage {
  items: Diagnostic[];
  next_offset: number | null;
  total: number;
}
export type SourcePurpose = 'dictionaries' | 'resources' | 'resource_directory';
export interface NativeBridge {
  getAppPreferences(): Promise<AppPreferences>;
  setAppLanguage(language: LanguagePreference): Promise<AppPreferences>;
  getAppVersion(): Promise<string>;
  checkForUpdates(): Promise<{ version: string | null }>;
  openReleasePage(): Promise<void>;
  onAboutRequested(callback: () => void): Promise<() => void>;
  readClipboardText(): Promise<string>;
  openSupportPage(): Promise<void>;
  openIssue(id?: string): Promise<void>;
  listFormats(): Promise<FormatDescriptor[]>;
  pickSources(purpose: SourcePurpose): Promise<ImportedSourceInfo[]>;
  pickOutput(): Promise<OutputChoice | null>;
  createJob(draft: JobDraft): Promise<JobSnapshot>;
  startJob(id: string): Promise<JobSnapshot>;
  provideArchivePassword(id: string, archiveId: string, password: string): Promise<JobSnapshot>;
  pauseDownload(id: string): Promise<JobSnapshot>;
  resumeDownload(id: string): Promise<JobSnapshot>;
  cancelJob(id: string): Promise<JobSnapshot>;
  deleteJob(id: string): Promise<void>;
  updateJob(id: string, update: JobOptionsUpdate): Promise<JobSnapshot>;
  resolveAdditionalDictionaries(
    id: string,
    offerId: string,
    convert: boolean,
  ): Promise<JobSnapshot[]>;
  getJobs(): Promise<JobSnapshot[]>;
  readDiagnostics(id: string, offset: number, limit: number): Promise<DiagnosticPage>;
  exportResult(id: string): Promise<JobSnapshot>;
  openResult(id: string): Promise<void>;
  getResultPath(id: string): Promise<string>;
  onSourceDrop(callback: (event: SourceDropEvent) => void): Promise<() => void>;
  onJobEvent(callback: (event: JobEvent) => void): Promise<() => void>;
}
export type SourceDropEvent =
  { kind: 'files'; files: ImportedSourceInfo[] } | { kind: 'error'; error: AppError };
