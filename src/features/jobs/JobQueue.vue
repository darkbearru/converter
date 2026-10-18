<script setup lang="ts">
import type { JobSnapshot } from '../../shared/contracts';
import { stateLabels } from '../../shared/format';
defineProps<{ jobs: JobSnapshot[]; selected: string | null }>();
defineEmits<{ select: [id: string]; add: [] }>();
</script>
<template>
  <aside class="queue" aria-label="Очередь конвертации">
    <div class="queue-heading">
      <h2>
        Задания <span>{{ jobs.length }}</span>
      </h2>
      <button type="button" class="text-button" @click="$emit('add')">Добавить</button>
    </div>
    <p v-if="!jobs.length" class="queue-empty">
      Здесь появятся ваши словари и результаты конвертации.
    </p>
    <ul v-else class="queue-list">
      <li v-for="job in jobs" :key="job.id">
        <button
          type="button"
          class="queue-job"
          :aria-pressed="selected === job.id"
          @click="$emit('select', job.id)"
        >
          <strong>{{ job.display_name }}</strong
          ><span class="job-status" :data-state="job.state">{{ stateLabels[job.state] }}</span>
        </button>
      </li>
    </ul>
  </aside>
</template>
