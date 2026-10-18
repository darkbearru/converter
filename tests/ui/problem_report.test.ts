import { expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/vue';
import App from '../../src/App.vue';
import { createHarness } from './harness/bridge';

it('кнопки в шапке и истории открывают общий отчёт и отчёт нужной задачи', async () => {
  const { bridge } = createHarness('failed');
  const openIssue = vi.fn(async (_id?: string) => {});
  Object.assign(bridge, { openIssue });
  render(App, { props: { bridge } });
  const header = screen.getByRole('banner');
  await fireEvent.click(within(header).getByRole('button', { name: 'Сообщить о проблеме' }));
  expect(openIssue).toHaveBeenLastCalledWith(undefined);
  await fireEvent.click(await screen.findByRole('button', { name: /Dictionary.dsl/ }));
  const actions = screen.getByRole('group', { name: 'Действия с конвертацией' });
  await fireEvent.click(within(actions).getByRole('button', { name: 'Сообщить о проблеме' }));
  expect(openIssue).toHaveBeenLastCalledWith((await bridge.getJobs())[0]!.id);
});

it('предотвращает повторное открытие и позволяет повторить после ошибки браузера', async () => {
  const { bridge } = createHarness(null);
  let fail: (error: Error) => void = () => {};
  const openIssue = vi.fn(
    () =>
      new Promise<void>((_resolve, reject) => {
        fail = reject;
      }),
  );
  Object.assign(bridge, { openIssue });
  render(App, { props: { bridge } });
  const button = screen.getByRole('button', { name: 'Сообщить о проблеме' });
  await fireEvent.click(button);
  expect(button).toBeDisabled();
  await fireEvent.click(button);
  expect(openIssue).toHaveBeenCalledTimes(1);
  fail(new Error('Не удалось открыть браузер. Повторите попытку.'));
  expect(await screen.findByRole('alert')).toHaveTextContent('Не удалось открыть браузер');
  await waitFor(() => expect(button).toBeEnabled());
  openIssue.mockResolvedValueOnce();
  await fireEvent.click(button);
  await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument());
  expect(openIssue).toHaveBeenCalledTimes(2);
});
