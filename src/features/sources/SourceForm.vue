<script setup lang="ts">
import { displayPath } from '../../shared/format';
import { errorState } from '../../shared/i18n/errors';

import { useI18n } from 'vue-i18n';
const { t } = useI18n();

import { LocalizedError } from '../../shared/i18n/errors';

import { onMounted, ref, useId } from 'vue';
import type {
  FormatDescriptor,
  ImportedSourceInfo,
  JobSnapshot,
  NativeBridge,
  OutputChoice,
  SourceInput,
} from '../../shared/contracts';
import { formatBytes } from '../../shared/format';
const props = defineProps<{ bridge: NativeBridge }>();
const emit = defineEmits<{ created: [snapshot: JobSnapshot] }>();
const id = useId();
const mode = ref('files');
const sources = ref<ImportedSourceInfo[]>([]);
const resources = ref<ImportedSourceInfo[]>([]);
const url = ref('');
const sha = ref('');
const resourceUrls = ref('');
const encoding = ref('auto');
const title = ref('');
const description = ref('');
const output = ref<OutputChoice>('app_library');
const formats = ref<FormatDescriptor[]>([]);
const busy = ref(false);
const error = errorState();
onMounted(async () => {
  try {
    formats.value = await props.bridge.listFormats();
  } catch (e) {
    error.value = e;
  }
});
async function pick(purpose: 'dictionaries' | 'resources' | 'resource_directory') {
  error.value = '';
  try {
    const selected = await props.bridge.pickSources(purpose);
    if (purpose === 'dictionaries') sources.value = selected;
    else
      resources.value.push(
        ...selected.filter((f) => !resources.value.some((r) => r.import_id === f.import_id)),
      );
  } catch (e) {
    error.value = e;
  }
}
async function pickOutput() {
  try {
    const chosen = await props.bridge.pickOutput();
    if (chosen) output.value = chosen;
  } catch (e) {
    error.value = e;
  }
}
function validateUrl(value: string): string {
  let parsed: URL;
  try {
    parsed = new URL(value);
  } catch {
    throw new LocalizedError('app.url_required');
  }
  if (!['http:', 'https:'].includes(parsed.protocol) || parsed.username || parsed.password)
    throw new LocalizedError('app.url_credentials');
  return value.trim();
}
async function submit() {
  if (busy.value) return;
  error.value = '';
  busy.value = true;
  try {
    let main: SourceInput[];
    if (mode.value === 'url') {
      const validUrl = validateUrl(url.value.trim());
      if (sha.value.trim() && !/^[a-fA-F0-9]{64}$/.test(sha.value.trim()))
        throw new LocalizedError('app.sha_invalid');
      main = [
        { kind: 'remote', url: validUrl, expected_sha256: sha.value.trim().toLowerCase() || null },
      ];
    } else {
      if (!sources.value.length) throw new LocalizedError('app.choose_at_least_one');
      main = sources.value.map((f) => ({ kind: 'local', import_id: f.import_id }));
    }
    const attachments: SourceInput[] = resources.value.map((f) => ({
      kind: 'local',
      import_id: f.import_id,
    }));
    for (const line of resourceUrls.value
      .split('\n')
      .map((s) => s.trim())
      .filter(Boolean))
      attachments.push({ kind: 'remote', url: validateUrl(line), expected_sha256: null });
    // Удаляем только уже добавленные файлы: повтор отправки не создаёт дубликаты при частичной ошибке.
    for (const source of main) {
      const snapshot = await props.bridge.createJob({
        source,
        resources: attachments,
        output: output.value,
        options: { encoding: encoding.value === 'auto' ? 'auto' : { manual: encoding.value } },
        metadata_override:
          title.value.trim() || description.value.trim()
            ? {
                title: title.value.trim(),
                description: description.value.trim(),
                index_language: '',
                contents_language: '',
                source_encoding: '',
              }
            : null,
        auto_start: true,
      });
      if (source.kind === 'local')
        sources.value = sources.value.filter((f) => f.import_id !== source.import_id);
      emit('created', snapshot);
    }
  } catch (e) {
    error.value = e;
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <form class="source-form" novalidate @submit.prevent="submit">
    <header class="section-intro">
      <h2>{{ t('app.new_conversion') }}</h2>
      <p class="muted">{{ t('app.new_intro') }}</p>
    </header>
    <fieldset :disabled="busy" class="source-fields">
      <legend>{{ t('app.dictionary_source') }}</legend>
      <div class="segmented">
        <label
          ><input v-model="mode" type="radio" value="files" :name="`${id}-source`" />
          {{ t('app.local_files') }} </label
        ><label
          ><input v-model="mode" type="radio" value="url" :name="`${id}-source`" />
          {{ t('app.by_url') }}
        </label>
      </div>
      <div v-if="mode === 'files'" class="field-stack">
        <button type="button" @click="pick('dictionaries')">
          {{ t('app.choose_dictionaries') }}
        </button>
        <p class="hint">
          {{ formats.map((f) => f.name).join(', ') || 'DSL' }} ·
          {{
            formats
              .flatMap((f) => f.extensions)
              .map((ext) => `.${ext}`)
              .join(', ') || '.dsl, .dsl.dz'
          }}
          {{ t('app.zip_multiple') }}
        </p>
        <ul v-if="sources.length" class="file-list">
          <li v-for="file in sources" :key="file.import_id">
            <span>{{ file.display_name }}</span
            ><span v-if="file.bytes !== null" class="muted">{{ formatBytes(file.bytes) }}</span>
          </li>
        </ul>
      </div>
      <div v-else class="field-stack">
        <label :for="`${id}-url`"> {{ t('app.dictionary_url') }} </label
        ><input
          :id="`${id}-url`"
          v-model="url"
          type="url"
          spellcheck="false"
          autocomplete="off"
          placeholder="https://example.org/dictionary.zip"
        />
        <p class="hint">{{ t('app.direct_url_hint') }}</p>
        <label :for="`${id}-sha`"> {{ t('app.sha_optional') }} </label
        ><input
          :id="`${id}-sha`"
          v-model="sha"
          class="hash-input"
          spellcheck="false"
          autocomplete="off"
          maxlength="64"
          :placeholder="t('app.sha_placeholder')"
        />
      </div>
    </fieldset>
    <fieldset :disabled="busy" class="source-fields">
      <legend>{{ t('app.images_audio') }}</legend>
      <p class="hint">{{ t('app.resource_hint') }}</p>
      <div class="button-row">
        <button type="button" @click="pick('resources')">{{ t('app.choose_resources') }}</button
        ><button type="button" @click="pick('resource_directory')">
          {{ t('app.choose_resource_folder') }}
        </button>
      </div>
      <ul v-if="resources.length" class="file-list">
        <li v-for="file in resources" :key="file.import_id">
          <span>{{ file.display_name }}</span
          ><button
            type="button"
            class="text-button"
            :aria-label="t('app.remove_named', { name: file.display_name })"
            @click="resources = resources.filter((f) => f.import_id !== file.import_id)"
          >
            {{ t('app.remove') }}
          </button>
        </li>
      </ul>
      <label :for="`${id}-resources`"> {{ t('app.resource_urls') }} </label
      ><textarea
        :id="`${id}-resources`"
        v-model="resourceUrls"
        rows="2"
        spellcheck="false"
        :placeholder="t('app.resource_url_placeholder')"
      />
    </fieldset>
    <fieldset :disabled="busy" class="source-fields">
      <legend>{{ t('app.settings_result') }}</legend>
      <div class="field-grid">
        <div>
          <label :for="`${id}-encoding`"> {{ t('app.encoding') }} </label
          ><select :id="`${id}-encoding`" v-model="encoding">
            <option value="auto">{{ t('app.auto_encoding') }}</option>
            <option value="utf-8">UTF-8</option>
            <option value="utf-16le">UTF-16 LE</option>
            <option value="utf-16be">UTF-16 BE</option>
            <option value="windows-1251">Windows-1251</option>
          </select>
        </div>
        <div>
          <label :for="`${id}-title`"> {{ t('app.title_optional') }} </label
          ><input :id="`${id}-title`" v-model="title" :placeholder="t('app.from_header')" />
        </div>
      </div>
      <label :for="`${id}-description`"> {{ t('app.description_optional') }} </label
      ><textarea :id="`${id}-description`" v-model="description" rows="2" />
      <div class="destination">
        <div>
          <strong> {{ t('app.save_to') }} </strong>
          <p class="hint destination-path">
            {{
              output === 'app_library' ? t('app.app_library') : displayPath(output.desktop_folder)
            }}
          </p>
        </div>
        <button type="button" @click="pickOutput">{{ t('app.choose_folder') }}</button>
      </div>
      <p class="hint">{{ t('app.copy_later') }}</p>
    </fieldset>
    <p v-if="error" role="alert" class="error-message">{{ error }}</p>
    <footer class="form-footer">
      <span class="muted"> {{ t('app.output_formats') }} </span
      ><button type="submit" class="primary" :disabled="busy">
        {{ busy ? t('app.adding') : t('app.add_start') }}
      </button>
    </footer>
  </form>
</template>
