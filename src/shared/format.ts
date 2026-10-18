import type { JobState, Stage, StageProgress, TransferState } from './contracts';

const integer = new Intl.NumberFormat('ru-RU');
export const formatCount = (value: number) => integer.format(value);
function noun(value: number, forms: [string, string, string]): string {
  const last = Math.abs(value) % 100;
  return forms[
    last >= 11 && last <= 14 ? 2 : last % 10 === 1 ? 0 : last % 10 >= 2 && last % 10 <= 4 ? 1 : 2
  ];
}
export const formatQuantity = (value: number, forms: [string, string, string]) =>
  `${formatCount(value)} ${noun(value, forms)}`;
export function formatBytes(value: number): string {
  if (!Number.isFinite(value) || value < 0) return '—';
  const units = ['Б', 'КиБ', 'МиБ', 'ГиБ', 'ТиБ'];
  const index =
    value === 0 ? 0 : Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1);
  return `${new Intl.NumberFormat('ru-RU', { maximumFractionDigits: index ? 1 : 0 }).format(value / 1024 ** index)} ${units[index]}`;
}
export const stageLabels: Record<Stage, string> = {
  inspect: 'Подготовка источника',
  read: 'Чтение статей',
  sort: 'Сортировка ключей',
  write_mdx: 'Запись MDX',
  write_mdd: 'Запись MDD',
  validate: 'Проверка результата',
  publish: 'Сохранение результата',
};
export const stateLabels: Record<JobState, string> = {
  queued: 'В очереди',
  downloading: 'Скачивание',
  download_retry_wait: 'Ожидание повтора',
  download_paused: 'Скачивание на паузе',
  download_failed: 'Ошибка скачивания',
  importing: 'Импорт файлов',
  inspecting: 'Подготовка',
  awaiting_input: 'Нужны параметры',
  converting: 'Конвертация',
  validating: 'Проверка',
  publishing: 'Сохранение',
  succeeded: 'Готово',
  succeeded_with_warnings: 'Готово с предупреждениями',
  failed: 'Ошибка конвертации',
  cancelled: 'Отменено',
  interrupted: 'Прервано',
};
export const transferLabels: Record<TransferState, string> = {
  pending: 'Ожидает',
  downloading: 'Скачивается',
  retry_wait: 'Ожидание повтора',
  paused: 'На паузе',
  verifying: 'Проверка файла',
  completed: 'Скачано',
  failed: 'Ошибка',
  cancelled: 'Отменено',
};
export function stageCounter(stage: StageProgress): string {
  const format = stage.unit === 'bytes' ? formatBytes : formatCount;
  const count = stage.total ?? stage.completed;
  const unit =
    stage.unit === 'resources'
      ? ` ${noun(count, ['ресурс', 'ресурса', 'ресурсов'])}`
      : stage.unit === 'entries' && stage.stage === 'read'
        ? ` ${noun(count, ['статья', 'статьи', 'статей'])}`
        : stage.unit === 'entries' && ['sort', 'write_mdx'].includes(stage.stage)
          ? ` ${noun(count, ['ключ', 'ключа', 'ключей'])}`
          : '';
  return `${format(stage.completed)}${stage.total !== null ? ` из ${format(stage.total)}` : ''}${unit}`;
}
export function errorMessage(error: unknown): string {
  if (error && typeof error === 'object' && 'message' in error && typeof error.message === 'string')
    return error.message;
  return 'Не удалось выполнить действие. Повторите попытку.';
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
