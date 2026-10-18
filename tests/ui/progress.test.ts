import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/vue';
import DownloadProgress from '../../src/features/progress/DownloadProgress.vue';
import ConversionProgress from '../../src/features/progress/ConversionProgress.vue';
import fixtures from './fixtures/contracts.json';
import type { JobSnapshot } from '../../src/shared/contracts';
import { formatBytes } from '../../src/shared/format';

const jobs = fixtures as unknown as Record<string, JobSnapshot>;
describe('Раздельные этапы задания', () => {
  it('показывает привычные русские обозначения размеров файлов', () => {
    expect(formatBytes(0)).toBe('0 байт');
    expect(formatBytes(512)).toBe('512 байт');
    expect(formatBytes(1024)).toBe('1 Кб');
    expect(formatBytes(1024 ** 2)).toBe('1 Мб');
    expect(formatBytes(1024 ** 3)).toBe('1 Гб');
    expect(formatBytes(1024 ** 4)).toBe('1 Тб');
  });
  it('не выдаёт последнюю скорость завершённого файла за текущую', () => {
    render(DownloadProgress, { props: { progress: jobs.converting.download } });
    expect(screen.queryByText('2 Мб/с')).not.toBeInTheDocument();
  });
  it('обозначает завершённую конвертацию значением 100 для доступности', () => {
    render(ConversionProgress, { props: { progress: jobs.succeeded.conversion } });
    expect(screen.getByRole('progressbar', { name: 'Конвертация' })).toHaveAttribute(
      'aria-valuenow',
      '100',
    );
  });
  it('показывает 50% скачивания и ожидание конвертации', () => {
    render(DownloadProgress, { props: { progress: jobs.downloading.download } });
    render(ConversionProgress, { props: { progress: jobs.downloading.conversion } });
    expect(screen.getByRole('progressbar', { name: 'Скачивание' })).toHaveAttribute(
      'aria-valuenow',
      '50',
    );
    expect(screen.getByRole('progressbar', { name: 'Конвертация' })).not.toHaveAttribute(
      'aria-valuenow',
    );
    expect(screen.getByText('Ожидает скачивания и подготовки')).toBeVisible();
  });
  it('сохраняет завершённое скачивание при записи MDX', () => {
    render(DownloadProgress, { props: { progress: jobs.converting.download } });
    render(ConversionProgress, { props: { progress: jobs.converting.conversion } });
    expect(screen.getByRole('progressbar', { name: 'Скачивание' })).toHaveAttribute(
      'aria-valuenow',
      '100',
    );
    expect(screen.getByRole('progressbar', { name: 'Конвертация' })).toHaveAttribute(
      'aria-valuenow',
      '25',
    );
    expect(screen.getByText('Запись MDX')).toBeVisible();
  });
  it('не придумывает общий процент при одном неизвестном размере', () => {
    render(DownloadProgress, { props: { progress: jobs.unknown.download } });
    expect(screen.getByRole('progressbar', { name: 'Скачивание' })).not.toHaveAttribute(
      'aria-valuenow',
    );
    expect(screen.getByText('Общий размер неизвестен')).toBeVisible();
    expect(screen.getByText('8 Гб')).toBeVisible();
  });
  it('для локального файла не рисует загрузку с нуля', () => {
    render(DownloadProgress, { props: { progress: jobs.local.download } });
    expect(screen.getByText('Локальный файл')).toBeVisible();
    expect(screen.queryByRole('progressbar')).not.toBeInTheDocument();
    expect(screen.queryByText('0%')).not.toBeInTheDocument();
  });
  it('объясняет причину возобновления и сохраняет большие счётчики', () => {
    render(DownloadProgress, { props: { progress: jobs.paused.download } });
    expect(screen.getByText('Сервер изменил файл; загрузка начнётся заново')).toBeVisible();
    expect(screen.getByText('На паузе')).toBeVisible();
    expect(formatBytes(5 * 1024 ** 3)).toBe('5 Гб');
  });
});
