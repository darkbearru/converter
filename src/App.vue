<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue';
import Dialog from 'primevue/dialog';
import bookIcon from '../book.svg';
import Button from 'primevue/button';
import type { JobOptionsUpdate, JobSnapshot, NativeBridge } from './shared/contracts';
import { nativeBridge } from './shared/native';
import { isActive } from './shared/format';
import Icon from './shared/ui/Icon.vue';
import { createJobStore } from './features/jobs/store';
import SourceComposer from './features/composer/SourceComposer.vue';
import CompletedHistory from './features/jobs/CompletedHistory.vue';
import ActiveTasks from './features/jobs/ActiveTasks.vue';
import JobSettings from './features/jobs/JobSettings.vue';
import DownloadProgress from './features/progress/DownloadProgress.vue';
import ConversionProgress from './features/progress/ConversionProgress.vue';
import DiagnosticsList from './features/results/DiagnosticsList.vue';
const props = defineProps<{ bridge?: NativeBridge }>();
const bridge = props.bridge ?? nativeBridge;
const store = createJobStore(bridge);
const { error } = store;
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
    <header class="app-header">
      <div class="app-identity">
        <img class="app-logo" :src="bookIcon" alt="" width="40" height="36" />
        <h1>Converter</h1>
      </div>
      <span class="app-caption">DSL → MDX + MDD</span>
    </header>
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
    <Dialog
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
      :visible="Boolean(detail)"
      modal
      :header="detail?.display_name"
      class="app-dialog task-dialog"
      @update:visible="!$event && (selected = null)"
      ><template #closeicon><Icon name="close" /></template
      ><template v-if="detail"
        ><p v-if="detail.error" role="alert" class="error-message">{{ detail.error.message }}</p>
        <JobSettings
          v-if="detail.state === 'awaiting_input'"
          :key="detail.id"
          :job="detail"
          :bridge="bridge"
          :busy="Boolean(store.pending[detail.id])"
          @save="save" /><DownloadProgress :progress="detail.download" /><ConversionProgress
          :progress="detail.conversion" /><DiagnosticsList
          :key="detail.id"
          :id="detail.id"
          :count="detail.diagnostic_count"
          :bridge="bridge" /></template
    ></Dialog>
  </div>
</template>
