<script setup lang="ts">
import { useI18n } from 'vue-i18n';
const { t } = useI18n();

import { computed } from 'vue';
import type { ConversionJobProgress } from '../../shared/contracts';
import { formatCount, stageLabel, stageCounter } from '../../shared/format';
const props = defineProps<{ progress: ConversionJobProgress }>();
const percent = computed(() => {
  if (props.progress.status === 'succeeded') return 100;
  const stage = props.progress.current;
  return props.progress.status === 'running' && stage?.total
    ? Math.min(100, (stage.completed / stage.total) * 100)
    : undefined;
});
</script>

<template>
  <section class="phase" aria-labelledby="conversion-title">
    <div class="phase-heading">
      <h3 id="conversion-title">{{ t('app.conversion') }}</h3>
      <span class="status-text">{{ t(`app.conversion_states.${progress.status}`) }}</span>
    </div>
    <div
      class="meter"
      :class="{
        'is-indeterminate': progress.status === 'running' && percent === undefined,
        'is-complete': progress.status === 'succeeded',
      }"
      role="progressbar"
      :aria-label="t('app.conversion')"
      aria-valuemin="0"
      aria-valuemax="100"
      :aria-valuenow="percent"
      :aria-valuetext="
        progress.status === 'running' && progress.current
          ? `${t('app.conversion')}: ${stageLabel(progress.current.stage)}`
          : t(`app.conversion_states.${progress.status}`)
      "
      :style="{ '--fill': `${percent ?? 0}%` }"
    >
      <span />
    </div>
    <p v-if="progress.status === 'not_started'" class="muted">{{ t('app.waiting_download') }}</p>
    <template v-else-if="progress.current">
      <div class="counter-line">
        <strong>{{ stageLabel(progress.current.stage) }}</strong
        ><span>{{ stageCounter(progress.current) }}</span>
      </div>
      <p class="muted">
        {{ t('app.articles_label') }} {{ formatCount(progress.current.articles) }} ·
        {{ t('app.resources_label') }}
        {{ formatCount(progress.current.resources) }}
      </p>
    </template>
    <ul
      v-if="progress.completed_stages.length"
      class="completed-stages"
      :aria-label="t('app.completed_stages')"
    >
      <li v-for="stage in progress.completed_stages" :key="stage.stage">
        {{ stageLabel(stage.stage) }} <span> {{ t('app.stage_done') }} </span>
      </li>
    </ul>
    <p v-if="progress.status === 'interrupted'" class="notice-text">
      {{ t('app.conversion_interrupted') }}
    </p>
  </section>
</template>
