<script setup lang="ts">
import { useI18n } from 'vue-i18n';
const { t } = useI18n();

import { computed } from 'vue';
import type { DownloadJobProgress } from '../../shared/contracts';
import { formatBytes, formatCount, errorMessage, transferLabel } from '../../shared/format';
const props = defineProps<{ progress: DownloadJobProgress }>();
const percent = computed(() =>
  props.progress.total_bytes && props.progress.percent !== null
    ? Math.min(100, Math.max(0, props.progress.percent))
    : undefined,
);
const state = computed(() => {
  const files = props.progress.files;
  if (files.length && files.every((f) => f.state === 'completed'))
    return transferLabel('completed');
  return transferLabel(files.find((f) => f.state !== 'completed')?.state ?? 'pending');
});
</script>

<template>
  <section class="phase" aria-labelledby="download-title">
    <div class="phase-heading">
      <h3 id="download-title">{{ t('app.downloading') }}</h3>
      <span class="status-text">{{ progress.kind === 'local' ? t('app.local_file') : state }}</span>
    </div>
    <p v-if="progress.kind === 'local'" class="muted">{{ t('app.source_available') }}</p>
    <template v-else>
      <div
        class="meter"
        :class="{ 'is-indeterminate': percent === undefined }"
        role="progressbar"
        :aria-label="t('app.downloading')"
        aria-valuemin="0"
        aria-valuemax="100"
        :aria-valuenow="percent"
        :aria-valuetext="percent === undefined ? t('app.size_unknown') : `${Math.round(percent)}%`"
        :style="{ '--fill': `${percent ?? 0}%` }"
      >
        <span />
      </div>
      <div class="counter-line">
        <span
          >{{ formatBytes(progress.received_bytes)
          }}<template v-if="progress.total_bytes !== null">
            {{ ` ${t('app.of')} ${formatBytes(progress.total_bytes)}` }}</template
          ></span
        ><strong v-if="percent !== undefined">{{ Math.round(percent) }}%</strong
        ><span v-else class="muted"> {{ t('app.size_unknown') }} </span>
      </div>
      <ul class="transfer-list" :aria-label="t('app.download_files')">
        <li v-for="(file, index) in progress.files" :key="file.source_id">
          <div class="file-line">
            <span
              >{{ t('app.file_number', { number: formatCount(index + 1) }) }} ·
              {{ transferLabel(file.state) }}</span
            ><span v-if="file.state === 'downloading' && file.bytes_per_second !== null"
              >{{ formatBytes(file.bytes_per_second) }} {{ t('app.per_second') }}
            </span>
          </div>
          <p v-if="progress.files.length > 1" class="muted">
            {{ formatBytes(file.received_bytes)
            }}<template v-if="file.total_bytes !== null">
              {{ ` ${t('app.of')} ${formatBytes(file.total_bytes)}` }} ·
              {{
                Math.min(
                  100,
                  Math.round((file.received_bytes / Math.max(1, file.total_bytes)) * 100),
                )
              }}%</template
            ><template v-else> · {{ t('app.file_size_unknown') }}</template>
          </p>
          <p v-if="file.restart_reason" class="notice-text">
            {{ file.restart_message ? errorMessage(file.restart_message) : file.restart_reason }}
          </p>
        </li>
      </ul>
      <p v-if="progress.files.some((file) => file.state === 'paused')" class="muted">
        {{ t('app.saved_download', { size: formatBytes(progress.confirmed_bytes) }) }}
      </p>
    </template>
  </section>
</template>
