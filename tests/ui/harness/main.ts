import { mountApplication } from '../../../src/shared/bootstrap';
import { preferencesFor } from '../../../src/shared/i18n';
import '../../../src/styles/tokens.css';
import '../../../src/styles/app.css';
import { createHarness } from './bridge';

if (!import.meta.env.DEV) throw new Error('Harness доступен только в разработке');
const query = new URLSearchParams(location.search);
const harness = createHarness(query.get('scenario') ?? 'preview');
const lang = query.get('lang');
const initial = preferencesFor(
  lang === 'ru' || lang === 'en' ? lang : 'system',
  query.get('system') ?? 'ru-RU',
);
void mountApplication(harness.bridge, initial).then(() => {
  if (query.has('about')) harness.openAbout();
});
const controls = document.querySelector<HTMLElement>('[data-testid="harness-controls"]')!;
if (query.has('capture')) controls.hidden = true;
document.querySelector('#advance')!.addEventListener('click', harness.advance);
document.querySelector('#finish')!.addEventListener('click', harness.finish);
document.querySelector('#stale')!.addEventListener('click', harness.stale);
document.querySelector('#about')!.addEventListener('click', harness.openAbout);
