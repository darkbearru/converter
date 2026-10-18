<script setup lang="ts">
import { useId } from 'vue';
import type { ExportSnapshot, ResultInfo } from '../../shared/contracts';
import { formatBytes, formatQuantity } from '../../shared/format';
defineProps<{ result: ResultInfo; exportState: ExportSnapshot }>();
const titleId = useId();
</script>
<template>
  <section class="result-panel" :aria-labelledby="titleId">
    <h3 :id="titleId">Результат</h3>
    <p class="muted">
      {{ formatQuantity(result.summary.source_articles, ['статья', 'статьи', 'статей']) }} ·
      {{ formatQuantity(result.summary.output_keys, ['ключ', 'ключа', 'ключей']) }} ·
      {{ formatQuantity(result.summary.resources, ['ресурс', 'ресурса', 'ресурсов']) }}
    </p>
    <ul class="file-list">
      <li v-for="file in result.files" :key="file.name">
        <span>{{ file.name }}</span
        ><span class="muted">{{ formatBytes(file.bytes) }}</span>
      </li>
    </ul>
    <p v-if="exportState.error" role="alert" class="error-message">
      {{ exportState.error.message }} Готовый результат сохранён в приложении.
    </p>
    <p v-if="exportState.status === 'succeeded'" role="status" class="success-message">
      Копия сохранена. Файлов: {{ exportState.saved_files.length }}.
    </p>
  </section>
</template>
