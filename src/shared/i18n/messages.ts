import { text, catalogs, activeLocale } from './index';
import type { LocalizedMessage } from '../contracts';
export function formatMessage(value: LocalizedMessage): string {
  if (!value.message_key) return value.message;
  const find = (catalog: unknown): unknown =>
    value
      .message_key!.split('.')
      .reduce<unknown>(
        (current, key) =>
          current && typeof current === 'object'
            ? (current as Record<string, unknown>)[key]
            : undefined,
        catalog,
      );
  if (typeof find(catalogs[activeLocale()]) !== 'string' && typeof find(catalogs.en) !== 'string')
    return value.message;
  return text(value.message_key, value.message_args ?? {});
}
