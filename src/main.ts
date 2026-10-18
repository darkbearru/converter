import { createApp } from 'vue';
import { PrimeVue, primeVueOptions } from './shared/theme';
import App from './App.vue';
import './styles/tokens.css';
import './styles/app.css';
createApp(App).use(PrimeVue, primeVueOptions).mount('#app');
