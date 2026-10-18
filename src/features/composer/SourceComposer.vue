<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, useId, watch } from 'vue';
import FileUpload from 'primevue/fileupload';
import ProgressBar from 'primevue/progressbar';
import Dialog from 'primevue/dialog';
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
import { errorMessage, formatBytes, transferLabels } from '../../shared/format';
const props = defineProps<{ bridge: NativeBridge; jobs: JobSnapshot[] }>();
const emit = defineEmits<{ snapshot: [job: JobSnapshot]; submitted: [id: string] }>();
const id = useId();
const settings = reactive(defaultSettings());
const resourcesChanged = ref(false);
const files = ref<ImportedSourceInfo[]>([]);
const busy = ref(false);
const error = ref('');
const linkDialog = ref(false);
const settingsDialog = ref(false);
const url = ref('');
const sha = ref('');
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
    error.value = 'Выберите файл словаря. Папки ресурсов добавляются в настройках.';
    return;
  }
  files.value = ordinary;
  error.value = '';
}
onMounted(async () => {
  if (!props.bridge.onSourceDrop) return;
  try {
    const stop = await props.bridge.onSourceDrop((event) => {
      if (event.kind === 'error') error.value = event.error.message;
      else acceptFiles(event.files);
    });
    if (disposed) stop();
    else unsubscribe = stop;
  } catch (e) {
    if (!disposed) error.value = errorMessage(e);
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
    error.value = errorMessage(e);
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
  if (busy.value || remote.value) return;
  error.value = '';
  busy.value = true;
  try {
    const source: SourceInput = {
      kind: 'remote',
      url: directUrl(url.value),
      expected_sha256: sha.value.trim().toLowerCase() || null,
    };
    if (source.expected_sha256 && !/^[a-f0-9]{64}$/.test(source.expected_sha256))
      throw new Error('SHA-256 должен содержать 64 шестнадцатеричных символа');
    const job = await props.bridge.createJob(draft(source, true));
    emit('snapshot', job);
    remoteId.value = job.id;
    files.value = [];
    linkDialog.value = false;
    url.value = '';
    sha.value = '';
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
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
    error.value = errorMessage(e);
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
    error.value = errorMessage(e);
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
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
}
</script>
<template>
  <section class="source-composer" aria-label="Новый словарь" :aria-busy="busy">
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
            aria-label="Убрать выбранный источник"
            text
            rounded
            :disabled="busy"
            @click="clear"
            ><Icon name="close"
          /></Button>
          <Button
            class="icon-button"
            aria-label="Добавить ссылку"
            title="Скачать по ссылке"
            text
            rounded
            :disabled="busy || Boolean(remote)"
            @click="
              error = '';
              linkDialog = true;
            "
            ><Icon name="link"
          /></Button>
          <Button
            class="icon-button"
            aria-label="Настройки конвертации"
            title="Настройки"
            text
            rounded
            :disabled="busy"
            @click="settingsDialog = true"
            ><Icon name="settings"
          /></Button>
        </div>
      </template>
      <template #content>
        <div v-if="transferring && remote && progress" class="source-transfer">
          <ProgressBar
            aria-label="Скачивание"
            :mode="progress.percent === null ? 'indeterminate' : 'determinate'"
            :value="progress.percent ?? undefined"
          />
          <div class="transfer-caption" role="status">
            <span v-if="progress.total_bytes === null">Общий размер неизвестен</span>
            <span>{{
              progress.files[0] ? transferLabels[progress.files[0].state] : 'Подготовка скачивания'
            }}</span
            ><span
              >{{ formatBytes(progress.received_bytes)
              }}<template v-if="progress.total_bytes !== null">
                из {{ formatBytes(progress.total_bytes) }}</template
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
                  ? 'Продолжить скачивание'
                  : 'Приостановить скачивание'
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
              aria-label="Отменить скачивание"
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
            aria-label="Конвертировать"
            title="Конвертировать"
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
          aria-label="Выбрать словари"
          :disabled="busy"
          @click="pick"
        >
          <Icon name="upload" /><span>Перетащите файлы сюда или нажмите для просмотра</span>
        </button>
      </template>
    </FileUpload>
    <p v-if="error" role="alert" class="composer-error error-message">{{ error }}</p>
    <p v-if="remote?.error" role="alert" class="composer-error error-message">
      {{ remote.error.message }}
    </p>
    <Dialog
      v-model:visible="linkDialog"
      modal
      header="Скачать словарь"
      class="link-dialog app-dialog"
      ><template #closeicon><Icon name="close" /></template>
      <form id="source-link-form" class="dialog-fields" @submit.prevent="download">
        <div class="field-stack">
          <label :for="`${id}-url`">Ссылка на словарь</label
          ><InputText
            :id="`${id}-url`"
            v-model="url"
            type="url"
            required
            autofocus
            placeholder="https://example.org/dictionary.zip"
          />
        </div>
        <div class="field-stack">
          <label :for="`${id}-sha`">SHA-256 (необязательно)</label
          ><InputText :id="`${id}-sha`" v-model="sha" autocomplete="off" />
        </div>
        <p v-if="error" role="alert" class="error-message">{{ error }}</p>
      </form>
      <template #footer
        ><Button label="Отмена" text @click="linkDialog = false" /><Button
          type="submit"
          form="source-link-form"
          label="Скачать"
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
