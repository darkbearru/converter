<script setup lang="ts">
import { computed } from 'vue';
import type { DownloadJobProgress } from '../../shared/contracts';
import { formatBytes, transferLabels } from '../../shared/format';
const props = defineProps<{ progress: DownloadJobProgress }>();
const percent = computed(() =>
  props.progress.total_bytes && props.progress.percent !== null
    ? Math.min(100, Math.max(0, props.progress.percent))
    : undefined,
);
const state = computed(() => {
  const files = props.progress.files;
  if (files.length && files.every((f) => f.state === 'completed')) return 'Скачано';
  return transferLabels[files.find((f) => f.state !== 'completed')?.state ?? 'pending'];
});
</script>

<template>
  <section class="phase" aria-labelledby="download-title">
    <div class="phase-heading">
      <h3 id="download-title">Скачивание</h3>
      <span class="status-text">{{ progress.kind === 'local' ? 'Локальный файл' : state }}</span>
    </div>
    <p v-if="progress.kind === 'local'" class="muted">Источник доступен на устройстве.</p>
    <template v-else>
      <div
        class="meter"
        :class="{ 'is-indeterminate': percent === undefined }"
        role="progressbar"
        aria-label="Скачивание"
        aria-valuemin="0"
        aria-valuemax="100"
        :aria-valuenow="percent"
        :aria-valuetext="
          percent === undefined ? 'Общий размер неизвестен' : `${Math.round(percent)}%`
        "
        :style="{ '--fill': `${percent ?? 0}%` }"
      >
        <span />
      </div>
      <div class="counter-line">
        <span
          >{{ formatBytes(progress.received_bytes)
          }}<template v-if="progress.total_bytes !== null">
            из {{ formatBytes(progress.total_bytes) }}</template
          ></span
        ><strong v-if="percent !== undefined">{{ Math.round(percent) }}%</strong
        ><span v-else class="muted">Общий размер неизвестен</span>
      </div>
      <ul class="transfer-list" aria-label="Файлы скачивания">
        <li v-for="(file, index) in progress.files" :key="file.source_id">
          <div class="file-line">
            <span>Файл {{ index + 1 }} · {{ transferLabels[file.state] }}</span
            ><span v-if="file.bytes_per_second !== null"
              >{{ formatBytes(file.bytes_per_second) }}/с</span
            >
          </div>
          <p v-if="progress.files.length > 1" class="muted">
            {{ formatBytes(file.received_bytes)
            }}<template v-if="file.total_bytes !== null">
              из {{ formatBytes(file.total_bytes) }} ·
              {{
                Math.min(
                  100,
                  Math.round((file.received_bytes / Math.max(1, file.total_bytes)) * 100),
                )
              }}%</template
            ><template v-else> · Размер неизвестен</template>
          </p>
          <p v-if="file.restart_reason" class="notice-text">{{ file.restart_reason }}</p>
        </li>
      </ul>
      <p v-if="state === 'На паузе'" class="muted">
        Подтверждено и сохранено: {{ formatBytes(progress.confirmed_bytes) }}. Продолжите
        скачивание, когда будете готовы.
      </p>
    </template>
  </section>
</template>
