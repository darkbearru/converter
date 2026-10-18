import { expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/vue';
import App from '../../src/App.vue';
import fixtures from './fixtures/contracts.json';
import type { JobSnapshot, NativeBridge } from '../../src/shared/contracts';

function context() {
  const parent = structuredClone(fixtures.converting) as JobSnapshot;
  parent.display_name = 'UniversalChRu.dsl.dz';
  const offer = {
    id: 'offer',
    main_name: 'Universal (Ch-Ru)',
    dictionaries: [{ id: 'phrase', display_name: 'PhraseBookRuCh.dsl.dz' }],
  };
  Object.assign(parent, { additional_dictionaries: offer });
  const child = structuredClone(parent);
  child.id = 'child';
  child.display_name = 'PhraseBookRuCh.dsl.dz';
  Object.assign(child, { additional_dictionaries: null });
  let listener: (event: { snapshot: JobSnapshot }) => void = () => {};
  const bridge = {
    getJobs: async () => [parent],
    onJobEvent: async (callback: typeof listener) => {
      listener = callback;
      return () => {};
    },
    onSourceDrop: async () => () => {},
    listFormats: async () => [],
    resolveAdditionalDictionaries: vi.fn(async (_id: string, _offerId: string, convert: boolean) =>
      convert ? [child] : [],
    ),
  } as unknown as NativeBridge;
  return {
    parent,
    child,
    offer,
    bridge,
    clearOffer: () =>
      listener({
        snapshot: { ...parent, sequence: parent.sequence + 1, additional_dictionaries: null },
      }),
  };
}

it.each([true, false])(
  'предлагает независимую базу, решение convert=%s сохраняется отдельно от основного',
  async (convert) => {
    const { parent, offer, bridge } = context();
    const view = render(App, { props: { bridge } });
    const dialog = await screen.findByRole('dialog', { name: 'Другие словари в архиве' });
    expect(within(dialog).getByText('Universal (Ch-Ru)')).toBeVisible();
    expect(within(dialog).getByText('PhraseBookRuCh.dsl.dz')).toBeVisible();
    await fireEvent.click(
      within(dialog).getByRole('button', { name: convert ? 'Конвертировать' : 'Пропустить' }),
    );
    await waitFor(() =>
      expect(bridge.resolveAdditionalDictionaries).toHaveBeenCalledWith(
        parent.id,
        offer.id,
        convert,
      ),
    );
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(view.container.querySelectorAll('.active-task')).toHaveLength(convert ? 2 : 1);
  },
);

it('ошибка ответа оставляет предложение и даёт повторить решение', async () => {
  const { bridge, clearOffer } = context();
  vi.mocked(bridge.resolveAdditionalDictionaries).mockImplementationOnce(async () => {
    clearOffer();
    throw new Error('Очередь недоступна');
  });
  render(App, { props: { bridge } });
  const dialog = await screen.findByRole('dialog', { name: 'Другие словари в архиве' });
  await fireEvent.click(within(dialog).getByRole('button', { name: 'Конвертировать' }));
  expect(await within(dialog).findByRole('alert')).toHaveTextContent('Очередь недоступна');
  await fireEvent.click(within(dialog).getByRole('button', { name: 'Конвертировать' }));
  await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  expect(bridge.resolveAdditionalDictionaries).toHaveBeenCalledTimes(2);
});

it('ошибку сохранения можно закрыть, сохранив доступ к основной задаче', async () => {
  const { bridge } = context();
  vi.mocked(bridge.resolveAdditionalDictionaries).mockRejectedValue(new Error('Диск недоступен'));
  const view = render(App, { props: { bridge } });
  const dialog = await screen.findByRole('dialog', { name: 'Другие словари в архиве' });
  await fireEvent.click(within(dialog).getByRole('button', { name: 'Конвертировать' }));
  await within(dialog).findByRole('alert');
  await fireEvent.click(within(dialog).getAllByRole('button', { name: 'Закрыть' }).at(0)!);
  await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  expect(bridge.resolveAdditionalDictionaries).toHaveBeenCalledTimes(1);
  expect(view.container.querySelectorAll('.active-task')).toHaveLength(1);
});
