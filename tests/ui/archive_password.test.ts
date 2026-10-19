import { expect, it } from 'vitest';
import { fireEvent, render, screen, within, waitFor } from '@testing-library/vue';
import App from '../../src/App.vue';
import { createHarness } from './harness/bridge';

it('запрашивает пароль ZIP, отмена сохраняет задание и разрешает открыть ввод снова', async () => {
  const harness = createHarness('archive-password');
  render(App, { props: { bridge: harness.bridge } });
  const dialog = await screen.findByRole('dialog', { name: 'Пароль архива' });
  const input = within(dialog).getByLabelText('Пароль');
  expect(input).toHaveAttribute('type', 'password');
  await fireEvent.update(input, 'private-value');
  await fireEvent.click(within(dialog).getByRole('button', { name: 'Отмена' }));
  await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  await fireEvent.click(screen.getByRole('button', { name: /locked.zip/ }));
  await fireEvent.click(screen.getByRole('button', { name: 'Уточнить параметры' }));
  const reopened = await screen.findByRole('dialog', { name: 'Пароль архива' });
  expect(within(reopened).getByLabelText('Пароль')).toHaveValue('');
});
