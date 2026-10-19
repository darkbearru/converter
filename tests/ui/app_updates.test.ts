import { expect, it } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/vue';
import App from '../../src/App.vue';
import { createHarness } from './harness/bridge';

it('предлагает новый релиз один раз при запуске, сведения остаются в окне о программе', async () => {
  const harness = createHarness('update-available');
  render(App, { props: { bridge: harness.bridge } });
  const notice = await screen.findByRole('dialog', { name: 'Доступно обновление' });
  expect(within(notice).getByText(/0\.3\.0/)).toBeVisible();
  await fireEvent.click(within(notice).getByRole('button', { name: 'Позже' }));
  harness.openAbout();
  const about = await screen.findByRole('dialog', { name: 'О программе' });
  expect(within(about).getByText('© Alexey Abramenko')).toBeVisible();
  expect(within(about).getByText('Доступна версия 0.3.0')).toBeVisible();
  await fireEvent.click(within(about).getByRole('button', { name: 'Проверить обновления' }));
  await waitFor(() =>
    expect(screen.queryByRole('dialog', { name: 'Доступно обновление' })).toBeNull(),
  );
  await fireEvent.click(within(about).getByRole('button', { name: 'Скачать' }));
  expect(harness.openedRelease()).toBe(true);
});

it('ошибка проверки не прерывает запуск, ручная проверка может восстановиться', async () => {
  const harness = createHarness('update-offline');
  render(App, { props: { bridge: harness.bridge } });
  await waitFor(() => expect(harness.updateChecks()).toBe(1));
  expect(screen.queryByRole('dialog')).toBeNull();
  harness.openAbout();
  const about = await screen.findByRole('dialog', { name: 'О программе' });
  expect(await within(about).findByText('Не удалось проверить обновления')).toBeVisible();
  harness.restoreUpdates();
  await fireEvent.click(within(about).getByRole('button', { name: 'Проверить обновления' }));
  expect(await within(about).findByText('Установлена последняя версия')).toBeVisible();
});
