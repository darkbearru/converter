import PrimeVue from 'primevue/config';
import Aura from '@primeuix/themes/aura';
import { definePreset } from '@primeuix/themes';

const ConverterPreset = definePreset(Aura, {
  semantic: {
    primary: {
      50: '#fff3ed',
      100: '#ffe2d4',
      200: '#ffc1a8',
      300: '#ff9876',
      400: '#ff7950',
      500: '#F96133',
      600: '#dd4621',
      700: '#b73318',
      800: '#922c1b',
      900: '#76291b',
      950: '#401107',
    },
    colorScheme: {
      light: {
        primary: {
          color: '{primary.500}',
          contrastColor: '{surface.950}',
          hoverColor: '{primary.400}',
          activeColor: '{primary.300}',
        },
        surface: {
          0: '#ffffff',
          50: '{zinc.50}',
          100: '{zinc.100}',
          200: '{zinc.200}',
          300: '{zinc.300}',
          400: '{zinc.400}',
          500: '{zinc.500}',
          600: '{zinc.600}',
          700: '{zinc.700}',
          800: '{zinc.800}',
          900: '{zinc.900}',
          950: '{zinc.950}',
        },
      },
      dark: {
        primary: {
          color: '{primary.500}',
          contrastColor: '{surface.950}',
          hoverColor: '{primary.400}',
          activeColor: '{primary.300}',
        },
        surface: {
          0: '#ffffff',
          50: '{zinc.50}',
          100: '{zinc.100}',
          200: '{zinc.200}',
          300: '{zinc.300}',
          400: '{zinc.400}',
          500: '{zinc.500}',
          600: '{zinc.600}',
          700: '{zinc.700}',
          800: '{zinc.800}',
          900: '{zinc.900}',
          950: '{zinc.950}',
        },
      },
    },
  },
  components: {
    message: {
      colorScheme: {
        light: { warn: { color: '{yellow.800}' } },
      },
    },
    button: {
      colorScheme: {
        light: {
          root: {
            danger: {
              background: '{red.600}',
              hoverBackground: '{red.700}',
              activeBackground: '{red.800}',
              borderColor: '{red.600}',
              hoverBorderColor: '{red.700}',
              activeBorderColor: '{red.800}',
              focusRing: { color: '{red.600}' },
            },
          },
          outlined: {
            primary: { color: '{primary.700}' },
            secondary: { color: '{surface.600}' },
            danger: { color: '{red.700}' },
          },
          text: { primary: { color: '{primary.700}' } },
        },
      },
    },
  },
});
export const primeVueOptions = {
  theme: { preset: ConverterPreset, options: { darkModeSelector: 'system' } },
  locale: {
    fileSizeTypes: ['байт', 'Кб', 'Мб', 'Гб', 'Тб'],
    choose: 'Выбрать',
    upload: 'Загрузить',
    cancel: 'Отмена',
    completed: 'Готово',
    pending: 'Ожидает',
    emptyMessage: 'Нет вариантов',
    emptySearchMessage: 'Ничего не найдено',
    aria: {
      close: 'Закрыть',
      firstPageLabel: 'Первая страница',
      lastPageLabel: 'Последняя страница',
      prevPageLabel: 'Предыдущая страница',
      nextPageLabel: 'Следующая страница',
    },
  },
};
export { PrimeVue };
