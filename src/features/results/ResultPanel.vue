<script setup lang="ts">
import { errorMessage } from '../../shared/format';
import { useI18n } from 'vue-i18n';
const { t } = useI18n();

import type { ExportSnapshot, ResultInfo } from '../../shared/contracts';
import { formatBytes, formatQuantity, formatCount } from '../../shared/format';
defineProps<{ result: ResultInfo; exportState: ExportSnapshot }>();
</script>
<template>
  <section class="result-panel" :aria-label="t('app.result')">
    <p class="muted">
      {{ formatQuantity(result.summary.source_articles, 'articles') }} ·
      {{ formatQuantity(result.summary.output_keys, 'keys') }} ·
      {{ formatQuantity(result.summary.resources, 'resources') }}
    </p>
    <ul class="file-list">
      <li v-for="file in result.files" :key="file.name">
        <span>{{ file.name }}</span
        ><span class="muted">{{ formatBytes(file.bytes) }}</span>
      </li>
    </ul>
    <p v-if="exportState.error" role="alert" class="error-message">
      {{ errorMessage(exportState.error) }} {{ t('app.result_kept') }}
    </p>
    <p v-if="exportState.status === 'succeeded'" role="status" class="success-message">
      {{ t('app.copy_saved', { count: formatCount(exportState.saved_files.length) }) }}
    </p>
  </section>
</template>
