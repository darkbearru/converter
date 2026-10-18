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
      startJob: vi.fn(async () => snapshot),
      updateJob: vi.fn(async () => snapshot),
      cancelJob: vi.fn(async () => ({ ...snapshot, state: 'cancelled' })),
      onSourceDrop: vi.fn(async (callback) => {
        drop = callback;
        return () => {};
      }),
    } as unknown as NativeBridge,
    drop: (event: SourceDropEvent) => drop?.(event),
  };
}
describe('Нижняя панель источника', () => {
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
      expect(native.createJob).toHaveBeenCalledWith(
        expect.objectContaining({
          prepare_only: false,
          auto_start: true,
          source: { kind: 'local', import_id: 'local-1' },
        }),
      ),
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
      expect(adapter.native.createJob).toHaveBeenCalledWith(
        expect.objectContaining({ source: { kind: 'local', import_id: 'drop-1' } }),
      ),
    );
  });
});
