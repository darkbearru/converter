<script setup lang="ts">
import type { ExportSnapshot, ResultInfo } from '../../shared/contracts';
import { formatBytes, formatCount } from '../../shared/format';
defineProps<{ result: ResultInfo; exportState: ExportSnapshot; busy: boolean }>();
defineEmits<{ open: []; export: [] }>();
</script>
<template>
  <section class="result-panel" aria-labelledby="result-title">
    <h3 id="result-title">Результат</h3>
    <p class="muted">
      {{ formatCount(result.summary.source_articles) }} статей ·
      {{ formatCount(result.summary.output_keys) }} ключей ·
      {{ formatCount(result.summary.resources) }} ресурсов
    </p>
    <ul class="file-list">
      <li v-for="file in result.files" :key="file.name">
        <span>{{ file.name }}</span
        ><span class="muted">{{ formatBytes(file.bytes) }}</span>
      </li>
    </ul>
    <div class="button-row">
      <button
        v-if="result.can_open"
        type="button"
        class="primary"
        :disabled="busy"
        @click="$emit('open')"
      >
        Открыть папку результата</button
      ><button
        v-if="result.can_export"
        type="button"
        :disabled="busy || exportState.status === 'running'"
        @click="$emit('export')"
      >
        {{ exportState.status === 'running' ? 'Сохранение…' : 'Сохранить копию' }}
      </button>
    </div>
    <p v-if="exportState.error" role="alert" class="error-message">
      {{ exportState.error.message }} Готовый результат сохранён в приложении.
    </p>
    <p v-if="exportState.status === 'succeeded'" role="status" class="success-message">
      Копия сохранена. Файлов: {{ exportState.saved_files.length }}.
    </p>
  </section>
</template>
