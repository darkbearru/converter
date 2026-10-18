import { createApp } from 'vue';
import PrimeVue from 'primevue/config';
import App from './App.vue';
import './styles/tokens.css';
import './styles/app.css';
createApp(App)
  .use(PrimeVue, { locale: { fileSizeTypes: ['байт', 'Кб', 'Мб', 'Гб', 'Тб'] } })
  .mount('#app');
