import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/vue';
import SourceComposer from '../../src/features/composer/SourceComposer.vue';
import type { JobSnapshot, NativeBridge, SourceDropEvent } from '../../src/shared/contracts';
import fixtures from './fixtures/contracts.json';

function bridge() {
  const snapshot = structuredClone(fixtures.downloading) as JobSnapshot;
  let drop: ((event: SourceDropEvent) => void) | undefined;
  return {
    native: {
      listFormats: vi.fn(async () => [{ id: 'dsl', name: 'DSL', extensions: ['dsl', 'dsl.dz'] }]),
      pickSources: vi.fn(async () => [
        {
          import_id: 'local-1',
          display_name: 'main.dsl',
          bytes: 25 * 1024 ** 2,
          is_directory: false,
        },
      ]),
      pickOutput: vi.fn(async () => ({ desktop_folder: '/selected/output' })),
      createJob: vi.fn(async () => snapshot),
      createJobs: vi.fn(async () => ({ jobs: [snapshot], included: [] })),
      startJob: vi.fn(async () => snapshot),
      updateJob: vi.fn(async () => snapshot),
      cancelJob: vi.fn(async () => ({ ...snapshot, state: 'cancelled' })),
      readClipboardText: vi.fn(async () => ' https://example.org/словарь.zip\n'),
      onSourceDrop: vi.fn(async (callback) => {
        drop = callback;
        return () => {};
      }),
    } as unknown as NativeBridge,
    drop: (event: SourceDropEvent) => drop?.(event),
  };
}
describe('Нижняя панель источника', () => {
  it('отправляет выбранные части одним набором и объясняет пропуск включений', async () => {
    const adapter = bridge();
    const native = adapter.native as NativeBridge & { createJobs: ReturnType<typeof vi.fn> };
    native.createJobs.mockResolvedValue({
      jobs: [structuredClone(fixtures.local)],
      included: [
        { main_name: '大БКРС - 260610', dictionary_names: ['dabkrs_2.dsl', 'dabkrs_3.dsl'] },
      ],
    });
    render(SourceComposer, { props: { bridge: native, jobs: [] } });
    await waitFor(() => expect(native.onSourceDrop).toHaveBeenCalled());
    adapter.drop({
      kind: 'files',
      files: [3, 1, 2].map((n) => ({
        import_id: `part-${n}`,
        display_name: `dabkrs_${n}.dsl`,
        bytes: 1024,
        is_directory: false,
      })),
    });
    await fireEvent.click(await screen.findByRole('button', { name: 'Конвертировать' }));
    const dialog = await screen.findByRole('dialog', { name: 'Словари уже включены' });
    expect(dialog).toHaveTextContent('大БКРС - 260610');
    expect(dialog).toHaveTextContent('dabkrs_2.dsl');
    expect(dialog).toHaveTextContent('dabkrs_3.dsl');
    expect(native.createJobs).toHaveBeenCalledWith(
      [3, 1, 2].map((n) =>
        expect.objectContaining({ source: { kind: 'local', import_id: `part-${n}` } }),
      ),
    );
    expect(native.createJob).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByText('Закрыть', { selector: '.p-button-label' }));
    await waitFor(() =>
      expect(
        screen.queryByRole('dialog', { name: 'Словари уже включены' }),
      ).not.toBeInTheDocument(),
    );
    expect(screen.getByRole('button', { name: 'Выбрать словари' })).toBeVisible();
  });
  it('вставляет ссылку из нативного буфера и ждёт команды скачать', async () => {
    const { native } = bridge();
    render(SourceComposer, { props: { bridge: native, jobs: [] } });
    await fireEvent.click(screen.getByRole('button', { name: 'Добавить ссылку' }));
    expect(screen.queryByLabelText('SHA-256 (необязательно)')).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Вставить из буфера обмена' }));
    await waitFor(() =>
      expect(screen.getByLabelText('Ссылка на словарь')).toHaveValue(
        'https://example.org/словарь.zip',
      ),
    );
    expect(native.createJob).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole('button', { name: 'Скачать' }));
    await waitFor(() =>
      expect(native.createJob).toHaveBeenCalledWith(
        expect.objectContaining({
          source: {
            kind: 'remote',
            url: 'https://example.org/%D1%81%D0%BB%D0%BE%D0%B2%D0%B0%D1%80%D1%8C.zip',
            expected_sha256: null,
          },
        }),
      ),
    );
  });
  it.each(['empty', 'unavailable'])(
    'сохраняет введённую ссылку при проблеме буфера: %s',
    async (failure) => {
      const { native } = bridge();
      if (failure === 'empty') vi.mocked(native.readClipboardText).mockResolvedValue(' \n ');
      else
        vi.mocked(native.readClipboardText).mockRejectedValue(new Error('Clipboard unavailable'));
      render(SourceComposer, { props: { bridge: native, jobs: [] } });
      await fireEvent.click(screen.getByRole('button', { name: 'Добавить ссылку' }));
      await fireEvent.update(
        screen.getByLabelText('Ссылка на словарь'),
        'https://example.org/old.dsl',
      );
      await fireEvent.click(screen.getByRole('button', { name: 'Вставить из буфера обмена' }));
      expect(
        await screen.findByText(
          failure === 'empty'
            ? 'В буфере обмена нет текста. Скопируйте ссылку и повторите.'
            : 'Не удалось прочитать буфер обмена. Вставьте ссылку с клавиатуры.',
        ),
      ).toBeVisible();
      expect(screen.getByLabelText('Ссылка на словарь')).toHaveValue('https://example.org/old.dsl');
      expect(native.createJob).not.toHaveBeenCalled();
    },
  );
  it('округляет видимый процент скачивания до целого', () => {
    const { native } = bridge();
    const job = structuredClone(fixtures.downloading) as JobSnapshot;
    job.prepare_only = true;
    job.download.percent = 37.456;
    render(SourceComposer, { props: { bridge: native, jobs: [job] } });
    expect(screen.getByRole('progressbar', { name: 'Скачивание' })).toHaveTextContent('37%');
    expect(screen.queryByText('37.456%')).not.toBeInTheDocument();
  });
  it('сохраняет прежние ресурсы готового задания при добавлении новых после перезапуска', async () => {
    const { native } = bridge();
    const job = {
      ...structuredClone(fixtures.local),
      state: 'ready',
      prepare_only: true,
      resource_attachments: [
        { source_id: 'cached-resource', display_name: 'images.zip', bytes: 1024 },
      ],
    } as unknown as JobSnapshot;
    render(SourceComposer, { props: { bridge: native, jobs: [job] } });
    await fireEvent.click(screen.getByRole('button', { name: 'Настройки конвертации' }));
    expect(await screen.findByText('images.zip')).toBeVisible();
    await fireEvent.click(screen.getByRole('button', { name: 'Добавить файлы ресурсов' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Готово' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Конвертировать' }));
    await waitFor(() =>
      expect(native.updateJob).toHaveBeenCalledWith(
        job.id,
        expect.objectContaining({
          retained_resource_ids: ['cached-resource'],
          resource_sources: [{ kind: 'local', import_id: 'local-1' }],
        }),
      ),
    );
  });
  it('ждёт нажатия конвертации после выбора и очищается после запуска', async () => {
    const { native } = bridge();
    render(SourceComposer, { props: { bridge: native, jobs: [] } });
    await fireEvent.click(screen.getByRole('button', { name: 'Выбрать словари' }));
    expect(await screen.findByText('main.dsl')).toBeVisible();
    expect(screen.getByText('25 Мб')).toBeVisible();
    expect(native.createJob).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole('button', { name: 'Конвертировать' }));
    await waitFor(() =>
      expect(native.createJobs).toHaveBeenCalledWith([
        expect.objectContaining({
          prepare_only: false,
          auto_start: true,
          source: { kind: 'local', import_id: 'local-1' },
        }),
      ]),
    );
    expect(await screen.findByRole('button', { name: 'Выбрать словари' })).toBeVisible();
  });
  it('скачивает URL отдельно от команды конвертации', async () => {
    const { native } = bridge();
    render(SourceComposer, { props: { bridge: native, jobs: [] } });
    await fireEvent.click(screen.getByRole('button', { name: 'Добавить ссылку' }));
    await fireEvent.update(
      screen.getByLabelText('Ссылка на словарь'),
      'https://example.org/main.dsl',
    );
    await fireEvent.click(screen.getByRole('button', { name: 'Скачать' }));
    await waitFor(() =>
      expect(native.createJob).toHaveBeenCalledWith(
        expect.objectContaining({ prepare_only: true, auto_start: true }),
      ),
    );
    expect(native.startJob).not.toHaveBeenCalled();
  });
  it('получает только ID и метаданные перетаскиваемого файла', async () => {
    const adapter = bridge();
    render(SourceComposer, { props: { bridge: adapter.native, jobs: [] } });
    await waitFor(() => expect(adapter.native.onSourceDrop).toHaveBeenCalled());
    adapter.drop({
      kind: 'files',
      files: [{ import_id: 'drop-1', display_name: 'drop.dsl', bytes: 1024, is_directory: false }],
    });
    expect(await screen.findByText('drop.dsl')).toBeVisible();
    await fireEvent.click(screen.getByRole('button', { name: 'Конвертировать' }));
    await waitFor(() =>
      expect(adapter.native.createJobs).toHaveBeenCalledWith([
        expect.objectContaining({ source: { kind: 'local', import_id: 'drop-1' } }),
      ]),
    );
  });
});
