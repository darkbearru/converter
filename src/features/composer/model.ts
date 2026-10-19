import { LocalizedError } from '../../shared/i18n/errors';
import type { ImportedSourceInfo, OutputChoice, ResourceAttachment } from '../../shared/contracts';
export interface ComposerSettings {
  encoding: string;
  output: OutputChoice;
  resources: ImportedSourceInfo[];
  retainedResources: ResourceAttachment[];
  resourceUrls: string;
  title: string;
  description: string;
}
export const defaultSettings = (): ComposerSettings => ({
  encoding: 'auto',
  output: 'app_library',
  resources: [],
  retainedResources: [],
  resourceUrls: '',
  title: '',
  description: '',
});
export function directUrl(value: string): string {
  let parsed: URL;
  try {
    parsed = new URL(value.trim());
  } catch {
    throw new LocalizedError('app.url_required');
  }
  if (!['http:', 'https:'].includes(parsed.protocol) || parsed.username || parsed.password)
    throw new LocalizedError('app.url_credentials');
  return parsed.href;
}
