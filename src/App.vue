<script setup lang="ts">
import { errorState } from './shared/i18n/errors';

import { useI18n } from 'vue-i18n';
const { t } = useI18n();

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
import { useLocalization } from './shared/i18n';
import AppDialogs from './features/updates/AppDialogs.vue';
import ArchivePasswordDialog from './features/jobs/ArchivePasswordDialog.vue';
const props = defineProps<{ bridge?: NativeBridge }>();
const bridge = props.bridge ?? nativeBridge;
const localization = useLocalization();
const store = createJobStore(bridge);
const { error } = store;
const openingSupport = ref(false);
const supportError = errorState();
const issueError = errorState();
async function openSupportPage() {
  if (openingSupport.value) return;
  openingSupport.value = true;
  supportError.value = '';
  try {
    await bridge.openSupportPage();
  } catch (e) {
    supportError.value = e;
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
const dismissedPasswords = ref(new Set<string>());
const passwordError = ref<unknown>(null);
const passwordKey = (job: JobSnapshot) => `${job.id}:${job.generation}`;
const needsPassword = (job: JobSnapshot) =>
  job.state === 'awaiting_input' &&
  ['archive_password_required', 'archive_password_invalid'].includes(job.error?.code ?? '');
const passwordJob = computed(
  () =>
    jobs.value.find(
      (job) => needsPassword(job) && !dismissedPasswords.value.has(passwordKey(job)),
    ) ?? null,
);
function dismissPassword() {
  if (passwordJob.value) dismissedPasswords.value.add(passwordKey(passwordJob.value));
  passwordError.value = null;
}
function repairJob(id: string) {
  const job = store.jobs[id];
  if (job && needsPassword(job)) {
    dismissedPasswords.value.delete(passwordKey(job));
    passwordError.value = null;
  } else selected.value = id;
}
async function submitPassword(password: string) {
  const job = passwordJob.value;
  const archiveId = job?.error?.message_args?.archive_id;
  if (!job || typeof archiveId !== 'string') return;
  passwordError.value = null;
  if (!(await store.provideArchivePassword(job.id, archiveId, password)))
    passwordError.value = error.source.value;
}

const deleting = ref<JobSnapshot | null>(null);
const deleteError = errorState();
const additionalError = errorState();
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
  else additionalError.value = error.source.value;
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
  else deleteError.value = error.source.value;
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
    <AppDialogs
      :bridge="bridge"
      :busy="Boolean(selected || deleting || presentedOffer || passwordJob)"
    />
    <header class="app-header">
      <div class="app-identity">
        <img class="app-logo" :src="bookIcon" alt="" width="40" height="36" />
        <h1>Converter</h1>
      </div>
      <div class="app-header-actions">
        <span class="app-caption">DSL → MDX + MDD</span>
        <Button
          v-if="localization.preferences.support_url"
          class="icon-button support-button"
          text
          :aria-label="t('app.support')"
          :title="t('app.support')"
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
          @repair="repairJob"
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
    <ArchivePasswordDialog
      :job="passwordJob"
      :busy="Boolean(passwordJob && store.pending[passwordJob.id])"
      :error="passwordError"
      @close="dismissPassword"
      @submit="submitPassword"
    />
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
      :header="t('app.delete_history')"
      class="app-dialog delete-dialog"
      :closable="!deleting || !store.pending[deleting.id]"
      :close-on-escape="!deleting || !store.pending[deleting.id]"
      @update:visible="!$event && (deleting = null)"
    >
      <template #closeicon><Icon name="close" /></template>
      <p>{{ t('app.confirm_delete') }}</p>
      <p class="delete-filename">{{ deleting?.display_name }}?</p>
      <p class="hint">{{ t('app.keep_files') }}</p>
      <p v-if="deleteError" class="error-message" role="alert">{{ deleteError }}</p>
      <template #footer>
        <Button
          :label="t('app.cancel')"
          outlined
          autofocus
          :disabled="Boolean(deleting && store.pending[deleting.id])"
          @click="deleting = null"
        />
        <Button
          :label="t('app.delete')"
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
        <Message v-if="detail.error" severity="error">{{ errorMessage(detail.error) }}</Message>
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
