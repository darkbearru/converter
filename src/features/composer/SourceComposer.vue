<script setup lang="ts">
import { errorState } from '../../shared/i18n/errors';

import { useI18n } from 'vue-i18n';
const { t } = useI18n();

import { LocalizedError } from '../../shared/i18n/errors';

import { computed, onBeforeUnmount, onMounted, reactive, ref, useId, watch } from 'vue';
import FileUpload from 'primevue/fileupload';
import ProgressBar from 'primevue/progressbar';
import Dialog from 'primevue/dialog';
import { dialogCloseButtonProps, focusDialogOpener } from '../../shared/ui/dialog';
import InputText from 'primevue/inputtext';
import Button from 'primevue/button';
import Icon from '../../shared/ui/Icon.vue';
import SettingsDialog from './SettingsDialog.vue';
import { defaultSettings, directUrl } from './model';
import type {
  ImportedSourceInfo,
  JobSnapshot,
  NativeBridge,
  SourceInput,
} from '../../shared/contracts';
import { errorMessage, formatBytes, transferLabel } from '../../shared/format';
const props = defineProps<{ bridge: NativeBridge; jobs: JobSnapshot[] }>();
const emit = defineEmits<{ snapshot: [job: JobSnapshot]; submitted: [id: string] }>();
const id = useId();
const settings = reactive(defaultSettings());
const resourcesChanged = ref(false);
const files = ref<ImportedSourceInfo[]>([]);
const busy = ref(false);
const error = errorState();
const linkDialog = ref(false);
const settingsDialog = ref(false);
const url = ref('');
const pasting = ref(false);
const clipboardError = errorState();
let clipboardRequest = 0;
watch(linkDialog, () => {
  clipboardRequest++;
  clipboardError.value = '';
  pasting.value = false;
});
const remoteId = ref<string | null>(null);
const terminal = ['succeeded', 'succeeded_with_warnings', 'cancelled', 'failed', 'interrupted'];
const remote = computed(
  () =>
    props.jobs.find((job) => job.id === remoteId.value && !terminal.includes(job.state)) ??
    props.jobs.find((job) => job.prepare_only && !terminal.includes(job.state)),
);
const transferring = computed(() => Boolean(remote.value && remote.value.state !== 'ready'));
const ready = computed(() => files.value.length > 0 || remote.value?.state === 'ready');
const progress = computed(() => remote.value?.download);
const downloadPercent = computed(() => {
  const percent = progress.value?.percent;
  return percent === null || percent === undefined
    ? undefined
    : Math.round(Math.min(100, Math.max(0, percent)));
});
watch(
  () => remote.value?.id,
  () => {
    resourcesChanged.value = false;
    settings.retainedResources = remote.value?.resource_attachments
      ? [...remote.value.resource_attachments]
      : [];
    if (!remote.value) return;
    settings.resources = [];
    settings.resourceUrls = '';
    settings.encoding =
      typeof remote.value.options.encoding === 'string'
        ? 'auto'
        : remote.value.options.encoding.manual;
    settings.output = remote.value.output ?? settings.output;
    settings.title = remote.value.metadata?.title ?? settings.title;
    settings.description = remote.value.metadata?.description ?? settings.description;
  },
  { immediate: true },
);
let disposed = false;
let unsubscribe: (() => void) | undefined;
function acceptFiles(selected: ImportedSourceInfo[]) {
  if (busy.value || remote.value) return;
  const ordinary = selected.filter((file) => !file.is_directory);
  if (!ordinary.length) {
    error.value = new LocalizedError('app.select_dictionary_error');
    return;
  }
  files.value = ordinary;
  error.value = '';
}
onMounted(async () => {
  if (!props.bridge.onSourceDrop) return;
  try {
    const stop = await props.bridge.onSourceDrop((event) => {
      if (event.kind === 'error') error.value = event.error;
      else acceptFiles(event.files);
    });
    if (disposed) stop();
    else unsubscribe = stop;
  } catch (e) {
    if (!disposed) error.value = e;
  }
});
onBeforeUnmount(() => {
  disposed = true;
  unsubscribe?.();
});
async function pick() {
  if (busy.value || remote.value) return;
  error.value = '';
  try {
    const selected = await props.bridge.pickSources('dictionaries');
    if (selected.length) acceptFiles(selected);
  } catch (e) {
    error.value = e;
  }
}
function resourceSources() {
  const resources: SourceInput[] = settings.resources.map((file) => ({
    kind: 'local',
    import_id: file.import_id,
  }));
  for (const line of settings.resourceUrls
    .split('\n')
    .map((v) => v.trim())
    .filter(Boolean))
    resources.push({ kind: 'remote', url: directUrl(line), expected_sha256: null });
  return resources;
}
function draft(source: SourceInput, prepareOnly: boolean) {
  return {
    source,
    resources: resourceSources(),
    output: settings.output,
    options: {
      encoding: settings.encoding === 'auto' ? ('auto' as const) : { manual: settings.encoding },
    },
    metadata_override:
      settings.title.trim() || settings.description.trim()
        ? {
            title: settings.title.trim(),
            description: settings.description.trim(),
            index_language: '',
            contents_language: '',
            source_encoding: '',
          }
        : null,
    auto_start: true,
    prepare_only: prepareOnly,
  };
}
async function download() {
  if (busy.value || pasting.value || remote.value) return;
  error.value = '';
  busy.value = true;
  try {
    const source: SourceInput = {
      kind: 'remote',
      url: directUrl(url.value),
      expected_sha256: null,
    };
    const job = await props.bridge.createJob(draft(source, true));
    emit('snapshot', job);
    remoteId.value = job.id;
    files.value = [];
    linkDialog.value = false;
    url.value = '';
  } catch (e) {
    error.value = e;
  } finally {
    busy.value = false;
  }
}
async function pasteUrl() {
  if (busy.value || pasting.value) return;
  const request = ++clipboardRequest;
  const previousUrl = url.value;
  clipboardError.value = '';
  pasting.value = true;
  try {
    const text = (await props.bridge.readClipboardText()).trim();
    if (request !== clipboardRequest || url.value !== previousUrl) return;
    if (!text) {
      clipboardError.value = new LocalizedError('app.clipboard_empty');
      return;
    }
    url.value = text;
    error.value = '';
  } catch {
    if (request === clipboardRequest)
      clipboardError.value = new LocalizedError('app.clipboard_failed');
  } finally {
    if (request === clipboardRequest) pasting.value = false;
  }
}
async function convert() {
  if (!ready.value || busy.value) return;
  busy.value = true;
  error.value = '';
  try {
    if (remote.value) {
      const current = remote.value;
      const metadata =
        settings.title.trim() || settings.description.trim() || current.metadata
          ? {
              title: settings.title.trim(),
              description: settings.description.trim(),
              index_language: '',
              contents_language: '',
              source_encoding: '',
            }
          : undefined;
      const updated = await props.bridge.updateJob(current.id, {
        options: {
          encoding: settings.encoding === 'auto' ? 'auto' : { manual: settings.encoding },
        },
        output: settings.output,
        metadata_override: metadata,
        resource_sources: resourcesChanged.value ? resourceSources() : undefined,
        retained_resource_ids: resourcesChanged.value
          ? settings.retainedResources.map((item) => item.source_id)
          : undefined,
      });
      emit('snapshot', updated);
      const job = await props.bridge.startJob(current.id);
      emit('snapshot', job);
      emit('submitted', current.id);
      remoteId.value = null;
    } else {
      for (const file of [...files.value]) {
        const job = await props.bridge.createJob(
          draft({ kind: 'local', import_id: file.import_id }, false),
        );
        files.value = files.value.filter((item) => item.import_id !== file.import_id);
        emit('snapshot', job);
        emit('submitted', job.id);
      }
    }
  } catch (e) {
    error.value = e;
  } finally {
    busy.value = false;
  }
}
async function clear() {
  if (busy.value) return;
  error.value = '';
  if (!remote.value) {
    files.value = [];
    return;
  }
  busy.value = true;
  try {
    emit('snapshot', await props.bridge.cancelJob(remote.value.id));
    remoteId.value = null;
  } catch (e) {
    error.value = e;
  } finally {
    busy.value = false;
  }
}
async function transferAction() {
  if (!remote.value || busy.value) return;
  busy.value = true;
  try {
    const job = ['download_paused', 'download_failed'].includes(remote.value.state)
      ? await props.bridge.resumeDownload(remote.value.id)
      : await props.bridge.pauseDownload(remote.value.id);
    emit('snapshot', job);
  } catch (e) {
    error.value = e;
  } finally {
    busy.value = false;
  }
}
</script>
<template>
  <section class="source-composer" :aria-label="t('app.new_dictionary')" :aria-busy="busy">
    <FileUpload
      name="dictionary"
      mode="advanced"
      custom-upload
      :multiple="true"
      :show-upload-button="false"
      :show-cancel-button="false"
      :dt="{ progressbar: { height: '24px' } }"
      :pt="{ input: { disabled: true, tabindex: -1, 'aria-hidden': true } }"
    >
      <template #header>
        <div class="composer-tools">
          <Button
            v-if="ready"
            class="icon-button clear-source"
            :aria-label="t('app.clear_source')"
            text
            rounded
            :disabled="busy"
            @click="clear"
            ><Icon name="close"
          /></Button>
          <Button
            class="icon-button"
            :aria-label="t('app.add_url')"
            :title="t('app.download_url')"
            text
            rounded
            :disabled="busy || Boolean(remote)"
            @click="
              focusDialogOpener($event);
              error = '';
              linkDialog = true;
            "
            ><Icon name="link"
          /></Button>
          <Button
            class="icon-button"
            :aria-label="t('app.conversion_settings')"
            :title="t('app.settings')"
            text
            rounded
            :disabled="busy"
            @click="
              focusDialogOpener($event);
              settingsDialog = true;
            "
            ><Icon name="settings"
          /></Button>
        </div>
      </template>
      <template #content>
        <div v-if="transferring && remote && progress" class="source-transfer">
          <ProgressBar
            :aria-label="t('app.downloading')"
            :mode="downloadPercent === undefined ? 'indeterminate' : 'determinate'"
            :value="downloadPercent"
          />
          <div class="transfer-caption" role="status">
            <span v-if="progress.total_bytes === null"> {{ t('app.size_unknown') }} </span>
            <span>{{
              progress.files[0] ? transferLabel(progress.files[0].state) : t('app.prepare_download')
            }}</span
            ><span
              >{{ formatBytes(progress.received_bytes)
              }}<template v-if="progress.total_bytes !== null">
                {{ t('app.of') }} {{ formatBytes(progress.total_bytes) }}</template
              ></span
            >
          </div>
          <div class="composer-actions">
            <Button
              v-if="
                [
                  'downloading',
                  'download_retry_wait',
                  'download_paused',
                  'download_failed',
                ].includes(remote.state)
              "
              class="transfer-pause icon-button"
              :aria-label="
                ['download_paused', 'download_failed'].includes(remote.state)
                  ? t('app.resume_download')
                  : t('app.pause_download')
              "
              text
              rounded
              :disabled="busy"
              @click="transferAction"
              ><Icon
                :name="
                  ['download_paused', 'download_failed'].includes(remote.state) ? 'play' : 'pause'
                " /></Button
            ><Button
              class="round-action transfer-action"
              :aria-label="t('app.cancel_download')"
              rounded
              :disabled="busy"
              @click="clear"
              ><Icon name="stop"
            /></Button>
          </div>
        </div>
        <div v-else-if="ready" class="source-ready" role="status">
          <div v-if="remote" class="selected-source">
            <span class="selected-name">{{ remote.display_name }}</span
            ><span class="selected-size">{{
              formatBytes(remote.source_bytes ?? remote.download.files[0]?.received_bytes ?? 0)
            }}</span>
          </div>
          <div v-for="file in files" :key="file.import_id" class="selected-source">
            <span class="selected-name">{{ file.display_name }}</span
            ><span class="selected-size">{{
              file.bytes === null ? '' : formatBytes(file.bytes)
            }}</span>
          </div>
          <Button
            class="round-action convert-action"
            :aria-label="t('app.convert')"
            :title="t('app.convert')"
            rounded
            :loading="busy"
            :disabled="busy"
            @click="convert"
            ><Icon name="play"
          /></Button>
        </div>
        <button
          v-else
          type="button"
          class="source-dropzone"
          :aria-label="t('app.choose_dictionaries')"
          :disabled="busy"
          @click="pick"
        >
          <Icon name="upload" /><span> {{ t('app.drop_files') }} </span>
        </button>
      </template>
    </FileUpload>
    <p v-if="error" role="alert" class="composer-error error-message">{{ error }}</p>
    <p v-if="remote?.error" role="alert" class="composer-error error-message">
      {{ errorMessage(remote.error) }}
    </p>
    <Dialog
      :close-button-props="dialogCloseButtonProps"
      v-model:visible="linkDialog"
      modal
      :header="t('app.download_dictionary')"
      class="link-dialog app-dialog"
      ><template #closeicon><Icon name="close" /></template>
      <form id="source-link-form" class="dialog-fields" @submit.prevent="download">
        <div class="field-stack">
          <label :for="`${id}-url`"> {{ t('app.dictionary_url') }} </label>
          <div class="url-input-control">
            <InputText
              :id="`${id}-url`"
              v-model="url"
              type="url"
              required
              autofocus
              placeholder="https://example.org/dictionary.zip"
              :aria-describedby="clipboardError ? `${id}-clipboard-error` : undefined"
              @update:model-value="clipboardError = ''"
            /><Button
              type="button"
              class="icon-button paste-url"
              text
              :aria-label="t('app.paste')"
              :title="t('app.paste')"
              :disabled="busy || pasting"
              :aria-busy="pasting"
              @click="pasteUrl"
              ><Icon name="paste"
            /></Button>
          </div>
        </div>
        <p v-if="clipboardError" :id="`${id}-clipboard-error`" role="alert" class="error-message">
          {{ clipboardError }}
        </p>
        <p v-if="error" role="alert" class="error-message">{{ error }}</p>
      </form>
      <template #footer
        ><Button :label="t('app.cancel')" text @click="linkDialog = false" /><Button
          type="submit"
          form="source-link-form"
          :label="t('app.download')"
          :disabled="pasting"
          :loading="busy" /></template
    ></Dialog>
    <SettingsDialog
      v-model:visible="settingsDialog"
      :settings="settings"
      :bridge="bridge"
      :resources-locked="transferring"
      @resources-change="resourcesChanged = true"
    />
  </section>
</template>
