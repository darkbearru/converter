export const dialogCloseButtonProps = {
  severity: 'secondary',
  outlined: true,
  text: false,
  rounded: false,
} as const;

// WebKit не фокусирует кнопку по клику мышью. Dialog запоминает текущий
// фокус при открытии, чтобы вернуть его после закрытия.
export function focusDialogOpener(event: Event) {
  (event.currentTarget as HTMLElement).focus();
}
