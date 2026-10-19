import { createApp } from 'vue';
import App from '../../../src/App.vue';
import { PrimeVue, primeVueOptions } from '../../../src/shared/theme';
import '../../../src/styles/tokens.css';
import '../../../src/styles/app.css';
import { createHarness } from './bridge';

if (!import.meta.env.DEV) throw new Error('Harness доступен только в разработке');
const query = new URLSearchParams(location.search);
const harness = createHarness(query.get('scenario') ?? 'preview');
createApp(App, { bridge: harness.bridge }).use(PrimeVue, primeVueOptions).mount('#app');
const controls = document.querySelector<HTMLElement>('[data-testid="harness-controls"]')!;
if (query.has('capture')) controls.hidden = true;
document.querySelector('#advance')!.addEventListener('click', harness.advance);
document.querySelector('#finish')!.addEventListener('click', harness.finish);
document.querySelector('#stale')!.addEventListener('click', harness.stale);
document.querySelector('#about')!.addEventListener('click', harness.openAbout);
if (query.has('about')) harness.openAbout();
