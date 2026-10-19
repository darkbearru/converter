<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import Dialog from 'primevue/dialog';
import Message from 'primevue/message';
import { dialogCloseButtonProps } from './shared/ui/dialog';
import bookIcon from '../book.svg';
import Button from 'primevue/button';
import type {
  AdditionalDictionariesOffer,
  JobOptionsUpdate,
  JobSnapshot,
  NativeBridge,
} from './shared/contracts';
import { nativeBridge } from './shared/native';
import { errorMessage, isActive } from './shared/format';
import Icon from './shared/ui/Icon.vue';
import ProblemReportButton from './shared/ui/ProblemReportButton.vue';
import { createJobStore } from './features/jobs/store';
import SourceComposer from './features/composer/SourceComposer.vue';
import CompletedHistory from './features/jobs/CompletedHistory.vue';
import ActiveTasks from './features/jobs/ActiveTasks.vue';
import JobSettings from './features/jobs/JobSettings.vue';
import DownloadProgress from './features/progress/DownloadProgress.vue';
import ConversionProgress from './features/progress/ConversionProgress.vue';
import DiagnosticsList from './features/results/DiagnosticsList.vue';
import AdditionalDictionariesDialog from './features/jobs/AdditionalDictionariesDialog.vue';
import AppDialogs from './features/updates/AppDialogs.vue';
const props = defineProps<{ bridge?: NativeBridge }>();
const bridge = props.bridge ?? nativeBridge;
const store = createJobStore(bridge);
const { error } = store;
const openingSupport = ref(false);
const supportError = ref('');
const issueError = ref('');
async function openSupportPage() {
  if (openingSupport.value) return;
  openingSupport.value = true;
  supportError.value = '';
  try {
    await bridge.openSupportPage();
  } catch (e) {
    supportError.value = errorMessage(e);
  } finally {
    openingSupport.value = false;
  }
}
const jobs = computed(() => Object.values(store.jobs));
const active = computed(() =>
  jobs.value.filter(
    (job) =>
      !job.prepare_only &&
      (isActive(job.state) || ['download_paused', 'download_failed'].includes(job.state)),
  ),
);
const history = computed(() =>
  jobs.value
    .filter((job) =>
      [
        'succeeded',
        'succeeded_with_warnings',
        'failed',
        'cancelled',
        'interrupted',
        'awaiting_input',
      ].includes(job.state),
    )
    .reverse(),
);
const selected = ref<string | null>(null);
const deleting = ref<JobSnapshot | null>(null);
const deleteError = ref('');
const additionalError = ref('');
const additionalJob = computed(() =>
  jobs.value.find(
    (job) =>
      job.additional_dictionaries && !store.resolvedOffers.has(job.additional_dictionaries.id),
  ),
);
const presentedOffer = ref<{ jobId: string; offer: AdditionalDictionariesOffer } | null>(null);
const additionalOffer = computed(() => presentedOffer.value?.offer);
watch(
  [additionalJob, presentedOffer],
  () => {
    if (!presentedOffer.value && additionalJob.value?.additional_dictionaries) {
      presentedOffer.value = {
        jobId: additionalJob.value.id,
        offer: additionalJob.value.additional_dictionaries,
      };
      additionalError.value = '';
    }
  },
  { immediate: true },
);
async function decideAdditional(convert: boolean) {
  const current = presentedOffer.value;
  if (!current || store.pending[`offer:${current.offer.id}`]) return;
  additionalError.value = '';
  const resolved = await store.resolveAdditional(current.jobId, current.offer.id, convert);
  if (presentedOffer.value?.offer.id !== current.offer.id) return;
  if (resolved) presentedOffer.value = null;
  else additionalError.value = error.value;
}
function dismissAdditional() {
  if (!presentedOffer.value) return;
  store.resolvedOffers.add(presentedOffer.value.offer.id);
  presentedOffer.value = null;
}
function requestDelete(id: string) {
  deleteError.value = '';
  deleting.value = store.jobs[id] ?? null;
}
async function confirmDelete() {
  if (!deleting.value) return;
  const id = deleting.value.id;
  const removed = await store.deleteJob(id);
  if (deleting.value?.id !== id) return;
  if (removed) deleting.value = null;
  else deleteError.value = error.value;
}
const detail = computed(() => (selected.value ? store.jobs[selected.value] : undefined));
onBeforeUnmount(store.dispose);
async function save(update: JobOptionsUpdate) {
  if (!detail.value) return;
  const id = detail.value.id;
  await store.updateJob(id, update);
  if (!error.value) {
    await store.startJob(id);
    if (selected.value === id) selected.value = null;
  }
}
</script>
<template>
  <div class="app-shell">
    <AppDialogs :bridge="bridge" :busy="Boolean(selected || deleting || presentedOffer)" />
    <header class="app-header">
      <div class="app-identity">
        <img class="app-logo" :src="bookIcon" alt="" width="40" height="36" />
        <h1>Converter</h1>
      </div>
      <div class="app-header-actions">
        <span class="app-caption">DSL → MDX + MDD</span>
        <Button
          class="icon-button support-button"
          text
          aria-label="Поддержать разработчика"
          title="Поддержать разработчика"
          :disabled="openingSupport"
          @click="openSupportPage"
          ><Icon name="cash"
        /></Button>
        <ProblemReportButton :bridge="bridge" @error="issueError = $event" />
      </div>
    </header>
    <p v-if="supportError" class="app-error error-message" role="alert">{{ supportError }}</p>
    <p v-if="issueError" class="app-error error-message" role="alert">{{ issueError }}</p>
    <main class="conversion-workspace" id="main-content">
      <div v-if="history.length || active.length" class="job-panels">
        <CompletedHistory
          v-if="history.length"
          :jobs="history"
          :bridge="bridge"
          :pending="store.pending"
          @open="store.openResult"
          @export="store.exportResult"
          @retry="store.startJob"
          @repair="selected = $event"
          @remove="requestDelete"
        />
        <ActiveTasks
          :jobs="active"
          :pending="store.pending"
          @cancel="store.cancelJob"
          @pause="store.pauseDownload"
          @resume="store.resumeDownload"
          @details="selected = $event"
        />
      </div>
      <div class="composer-workspace">
        <p v-if="error" role="alert" class="app-error error-message">{{ error }}</p>
        <SourceComposer :bridge="bridge" :jobs="jobs" @snapshot="store.mergeSnapshot" />
      </div>
    </main>
    <AdditionalDictionariesDialog
      :offer="additionalOffer"
      :busy="Boolean(additionalOffer && store.pending[`offer:${additionalOffer.id}`])"
      :error="additionalError"
      @decide="decideAdditional"
      @dismiss="dismissAdditional"
    />
    <Dialog
      :close-button-props="dialogCloseButtonProps"
      :visible="Boolean(deleting)"
      modal
      header="Удалить из истории"
      class="app-dialog delete-dialog"
      :closable="!deleting || !store.pending[deleting.id]"
      :close-on-escape="!deleting || !store.pending[deleting.id]"
      @update:visible="!$event && (deleting = null)"
    >
      <template #closeicon><Icon name="close" /></template>
      <p>Вы действительно хотите удалить</p>
      <p class="delete-filename">{{ deleting?.display_name }}?</p>
      <p class="hint">Исходный словарь и готовые файлы останутся на диске.</p>
      <p v-if="deleteError" class="error-message" role="alert">{{ deleteError }}</p>
      <template #footer>
        <Button
          label="Отмена"
          outlined
          autofocus
          :disabled="Boolean(deleting && store.pending[deleting.id])"
          @click="deleting = null"
        />
        <Button
          label="Удалить"
          severity="danger"
          :loading="Boolean(deleting && store.pending[deleting.id])"
          :disabled="Boolean(deleting && store.pending[deleting.id])"
          @click="confirmDelete"
        />
      </template>
    </Dialog>
    <Dialog
      :close-button-props="dialogCloseButtonProps"
      :visible="Boolean(detail)"
      modal
      :header="detail?.display_name"
      class="app-dialog task-dialog"
      @update:visible="!$event && (selected = null)"
      ><template #closeicon><Icon name="close" /></template>
      <div v-if="detail" class="dialog-fields" tabindex="-1" autofocus>
        <Message v-if="detail.error" severity="error">{{ detail.error.message }}</Message>
        <JobSettings
          v-if="detail.state === 'awaiting_input'"
          :key="detail.id"
          :job="detail"
          :bridge="bridge"
          :busy="Boolean(store.pending[detail.id])"
          @save="save"
        /><DownloadProgress :progress="detail.download" /><ConversionProgress
          :progress="detail.conversion"
        /><DiagnosticsList
          :key="detail.id"
          :id="detail.id"
          :count="detail.diagnostic_count"
          :bridge="bridge"
          :severity="detail.error ? 'error' : 'warn'"
        /></div
    ></Dialog>
  </div>
</template>
