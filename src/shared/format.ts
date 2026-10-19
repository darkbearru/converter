import type { JobState, Stage, StageProgress, TransferState, LocalizedMessage } from './contracts';
import { activeLocale, catalogs, text } from './i18n';
import { formatMessage } from './i18n/messages';
export const formatCount = (value: number) => new Intl.NumberFormat(activeLocale()).format(value);
export function formatQuantity(value: number, unit: 'articles' | 'keys' | 'resources'): string {
  return text(`app.${unit}`, { count: formatCount(value) }, value);
}
export function formatBytes(value: number): string {
  if (!Number.isFinite(value) || value < 0) return '—';
  const units = catalogs[activeLocale()].primevue.fileSizeTypes;
  const index =
    value === 0 ? 0 : Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1);
  const number = new Intl.NumberFormat(activeLocale(), {
    maximumFractionDigits: index ? 1 : 0,
  }).format(value / 1024 ** index);
  return `${number} ${units[index]}`;
}
export const stageLabel = (stage: Stage) => text(`app.stages.${stage}`);
export const stateLabel = (state: JobState) => text(`app.states.${state}`);
export const transferLabel = (state: TransferState) => text(`app.transfers.${state}`);
export function stageCounter(stage: StageProgress): string {
  const format = stage.unit === 'bytes' ? formatBytes : formatCount;
  const count = stage.total ?? stage.completed;
  const unit =
    stage.unit === 'resources'
      ? 'resources'
      : stage.unit === 'entries' && stage.stage === 'read'
        ? 'articles'
        : stage.unit === 'entries' && ['sort', 'write_mdx'].includes(stage.stage)
          ? 'keys'
          : null;
  const amount = `${format(stage.completed)}${stage.total !== null ? ` ${text('app.of')} ${format(stage.total)}` : ''}`;
  // Select the noun for the total while retaining the completed/total counters.
  return unit ? text(`app.${unit}`, { count: amount }, count) : amount;
}
export function errorMessage(error: unknown): string {
  if (error && typeof error === 'object' && 'message' in error && typeof error.message === 'string')
    return formatMessage(error as LocalizedMessage);
  if (typeof error === 'string') return error;
  return text('app.action_failed');
}
export const isActive = (state: JobState) =>
  [
    'queued',
    'downloading',
    'download_retry_wait',
    'importing',
    'inspecting',
    'converting',
    'validating',
    'publishing',
  ].includes(state);

// Только отображение: внутренний canonical Windows-путь остаётся неизменным.
export function displayPath(value: string): string {
  if (/^\\\\\?\\UNC\\/i.test(value)) return `\\\\${value.slice(8)}`;
  if (/^\\\\\?\\[a-z]:\\/i.test(value)) return value.slice(4);
  return value;
}
