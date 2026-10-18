import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/vue';
import SourceForm from '../../src/features/sources/SourceForm.vue';
import type { JobDraft, NativeBridge } from '../../src/shared/contracts';

function bridge() {
  return {
    listFormats: vi.fn(async () => [
      { id: 'dsl', name: 'ABBYY DSL', extensions: ['dsl', 'dsl.dz'] },
    ]),
    pickSources: vi.fn(async () => [
      { import_id: 'source-1', display_name: 'Словарь.dsl', bytes: 100, is_directory: false },
    ]),
    pickOutput: vi.fn(async () => ({ desktop_folder: '/safe/output' })),
    createJob: vi.fn(async (draft: JobDraft) => ({ id: 'created', ...draft })),
  } as unknown as NativeBridge;
}
describe('Добавление источника', () => {
  it('передаёт URL, контрольную сумму и ресурсы в одно задание', async () => {
    const native = bridge();
    render(SourceForm, { props: { bridge: native } });
    await fireEvent.click(screen.getByRole('radio', { name: 'По ссылке' }));
    await fireEvent.update(
      screen.getByLabelText('Ссылка на словарь'),
      'https://example.org/dictionary.zip',
    );
    await fireEvent.update(screen.getByLabelText('SHA-256 (необязательно)'), 'a'.repeat(64));
    await fireEvent.update(
      screen.getByLabelText('Ссылки на ресурсы'),
      'https://example.org/images.zip\nhttps://example.org/sound.zip',
    );
    await fireEvent.click(screen.getByRole('button', { name: 'Выбрать папку' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Добавить и начать' }));
    await waitFor(() => expect(native.createJob).toHaveBeenCalledOnce());
    expect(native.createJob).toHaveBeenCalledWith(
      expect.objectContaining({
        source: {
          kind: 'remote',
          url: 'https://example.org/dictionary.zip',
          expected_sha256: 'a'.repeat(64),
        },
        resources: [
          { kind: 'remote', url: 'https://example.org/images.zip', expected_sha256: null },
          { kind: 'remote', url: 'https://example.org/sound.zip', expected_sha256: null },
        ],
        output: { desktop_folder: '/safe/output' },
        auto_start: true,
      }),
    );
  });
  it('останавливает ошибочную ссылку до вызова Rust', async () => {
    const native = bridge();
    render(SourceForm, { props: { bridge: native } });
    await fireEvent.click(screen.getByRole('radio', { name: 'По ссылке' }));
    await fireEvent.update(screen.getByLabelText('Ссылка на словарь'), 'file:///etc/passwd');
    await fireEvent.click(screen.getByRole('button', { name: 'Добавить и начать' }));
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Укажите прямую ссылку HTTP или HTTPS',
    );
    expect(native.createJob).not.toHaveBeenCalled();
  });
  it('использует идентификатор выбранного файла и ручную кодировку', async () => {
    const native = bridge();
    render(SourceForm, { props: { bridge: native } });
    await fireEvent.click(screen.getByRole('button', { name: 'Выбрать словари' }));
    await screen.findByText('Словарь.dsl');
    await fireEvent.update(screen.getByLabelText('Кодировка DSL'), 'windows-1251');
    await fireEvent.click(screen.getByRole('button', { name: 'Добавить и начать' }));
    await waitFor(() => expect(native.createJob).toHaveBeenCalledOnce());
    expect(native.createJob).toHaveBeenCalledWith(
      expect.objectContaining({
        source: { kind: 'local', import_id: 'source-1' },
        options: { encoding: { manual: 'windows-1251' } },
      }),
    );
  });
});
