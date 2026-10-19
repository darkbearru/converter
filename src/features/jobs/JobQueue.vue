<script setup lang="ts">
import { useI18n } from 'vue-i18n';
const { t } = useI18n();

import type { JobSnapshot } from '../../shared/contracts';
import { stateLabel } from '../../shared/format';
defineProps<{ jobs: JobSnapshot[]; selected: string | null }>();
defineEmits<{ select: [id: string]; add: [] }>();
</script>
<template>
  <aside class="queue" :aria-label="t('app.queue')">
    <div class="queue-heading">
      <h2>
        {{ t('app.jobs') }} <span>{{ jobs.length }}</span>
      </h2>
      <button type="button" class="text-button" @click="$emit('add')">{{ t('app.add') }}</button>
    </div>
    <p v-if="!jobs.length" class="queue-empty">{{ t('app.queue_empty') }}</p>
    <ul v-else class="queue-list">
      <li v-for="job in jobs" :key="job.id">
        <button
          type="button"
          class="queue-job"
          :aria-pressed="selected === job.id"
          @click="$emit('select', job.id)"
        >
          <strong>{{ job.display_name }}</strong
          ><span class="job-status" :data-state="job.state">{{ stateLabel(job.state) }}</span>
        </button>
      </li>
    </ul>
  </aside>
</template>
