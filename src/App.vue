<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue';
import type { JobOptionsUpdate, JobSnapshot, NativeBridge } from './shared/contracts';
import { nativeBridge } from './shared/native';
import { isActive, stateLabels } from './shared/format';
import { createJobStore } from './features/jobs/store';
import JobQueue from './features/jobs/JobQueue.vue';
import JobSettings from './features/jobs/JobSettings.vue';
import SourceForm from './features/sources/SourceForm.vue';
import DownloadProgress from './features/progress/DownloadProgress.vue';
import ConversionProgress from './features/progress/ConversionProgress.vue';
import ResultPanel from './features/results/ResultPanel.vue';
import DiagnosticsList from './features/results/DiagnosticsList.vue';
const props = defineProps<{ bridge?: NativeBridge }>();
const bridge = props.bridge ?? nativeBridge;
const store = createJobStore(bridge);
const { error } = store;
const selected = ref<string | null>(null);
const adding = ref(true);
const jobs = computed(() => Object.values(store.jobs));
const job = computed(() => (selected.value ? store.jobs[selected.value] : undefined));
const busy = computed(() => (job.value ? store.pending[job.value.id] : false));
void store.ready.then(() => {
  if (jobs.value.length) {
    selected.value = jobs.value[0].id;
    adding.value = false;
  }
});
onBeforeUnmount(store.dispose);
function created(snapshot: JobSnapshot) {
  store.mergeSnapshot(snapshot);
  selected.value = snapshot.id;
  adding.value = false;
}
async function save(update: JobOptionsUpdate) {
  if (!job.value) return;
  const id = job.value.id;
  await store.updateJob(id, update);
  if (!error.value) await store.startJob(id);
}
</script>

<template>
  <div class="app-shell">
    <header class="app-header">
      <div class="app-identity">
        <svg aria-hidden="true" width="28" height="28" viewBox="0 0 28 28" fill="none">
          <path
            d="M4 5h8a3 3 0 0 1 3 3v15a4 4 0 0 0-4-3H4V5Z"
            stroke="currentColor"
            stroke-width="1.6"
          />
          <path
            d="M15 9h9v14h-5a4 4 0 0 0-4 0M7 9h4M7 12h4M19 13h2M19 16h2"
            stroke="currentColor"
            stroke-width="1.6"
          />
        </svg>
        <h1>Converter</h1>
      </div>
      <span class="app-caption">Словари в MDX + MDD</span>
    </header>
    <div class="workspace">
      <JobQueue
        :jobs="jobs"
        :selected="adding ? null : selected"
        @select="
          selected = $event;
          adding = false;
        "
        @add="adding = true"
      />
      <main id="main-content" class="main-content">
        <p v-if="error" role="alert" class="error-message app-error">{{ error }}</p>
        <SourceForm v-if="adding || !job" :bridge="bridge" @created="created" />
        <article v-else class="job-detail" aria-labelledby="job-title">
          <header class="job-heading">
            <div>
              <h2 id="job-title">{{ job.display_name }}</h2>
              <p class="job-status" :data-state="job.state" role="status">
                {{ stateLabels[job.state] }}
              </p>
            </div>
            <div class="button-row">
              <button
                v-if="['downloading', 'download_retry_wait'].includes(job.state)"
                type="button"
                :disabled="busy"
                @click="store.pauseDownload(job.id)"
              >
                Приостановить скачивание</button
              ><button
                v-if="['download_paused', 'download_failed'].includes(job.state)"
                type="button"
                class="primary"
                :disabled="busy"
                @click="store.resumeDownload(job.id)"
              >
                Продолжить скачивание</button
              ><button
                v-if="['failed', 'cancelled', 'interrupted'].includes(job.state)"
                type="button"
                class="primary"
                :disabled="busy"
                @click="store.startJob(job.id)"
              >
                Запустить заново</button
              ><button
                v-if="isActive(job.state) || job.state === 'download_paused'"
                type="button"
                :disabled="busy"
                @click="store.cancelJob(job.id)"
              >
                Отменить задание
              </button>
            </div>
          </header>
          <p v-if="job.error" role="alert" class="error-message">{{ job.error.message }}</p>
          <JobSettings
            v-if="job.state === 'awaiting_input'"
            :key="job.id"
            :job="job"
            :bridge="bridge"
            :busy="busy"
            @save="save"
          />
          <div class="progress-stack">
            <DownloadProgress :progress="job.download" /><ConversionProgress
              :progress="job.conversion"
            />
          </div>
          <ResultPanel
            v-if="job.result"
            :result="job.result"
            :export-state="job.export"
            :busy="busy"
            @open="store.openResult(job.id)"
            @export="store.exportResult(job.id)"
          /><DiagnosticsList
            :key="job.id"
            :id="job.id"
            :count="job.diagnostic_count"
            :bridge="bridge"
          />
        </article>
      </main>
    </div>
    <footer class="app-footer">
      <span>Локальная конвертация</span><span>Источники сохраняются · 0.1.0</span>
    </footer>
  </div>
</template>
