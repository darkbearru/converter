import { expect, test } from 'vitest';
import { createHarness } from './harness/bridge';

test('предпросмотр содержит две завершённые задачи и один доступный журнал предупреждения', async () => {
  const { bridge } = createHarness('preview');
  const jobs = await bridge.getJobs();
  expect(jobs).toHaveLength(2);
  const ready = jobs.find((job) => job.state === 'succeeded')!;
  const warning = jobs.find((job) => job.state === 'succeeded_with_warnings')!;
  expect(ready.diagnostic_count).toBe(0);
  expect(warning.diagnostic_count).toBe(1);
  expect(warning.result?.summary.warnings).toBe(1);
  const report = await bridge.readDiagnostics(warning.id, 0, 20);
  expect(report.total).toBe(1);
  expect(report.items[0]).toMatchObject({
    severity: 'warning',
    message: 'Не найдена статья для ссылки: пример',
  });
});
