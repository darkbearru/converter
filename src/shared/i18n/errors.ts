import { computed, shallowRef } from 'vue';
import { text } from './index';
import { errorMessage } from '../format';
export class LocalizedError extends Error {
  readonly message_key: string;
  readonly message_args: Record<string, string | number>;
  constructor(key: string, args: Record<string, string | number> = {}) {
    super(text(key, args));
    this.message_key = key;
    this.message_args = args;
  }
}
export function errorState() {
  const source = shallowRef<unknown>();
  return Object.assign(
    computed({
      get: () => (source.value ? errorMessage(source.value) : ''),
      set: (value: unknown) => {
        source.value = value;
      },
    }),
    { source },
  );
}
