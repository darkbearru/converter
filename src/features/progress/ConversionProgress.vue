<script setup lang="ts">
import { computed } from 'vue';
import type { ConversionJobProgress } from '../../shared/contracts';
import { formatCount, stageLabels, stageCounter } from '../../shared/format';
const props = defineProps<{ progress: ConversionJobProgress }>();
const percent = computed(() => {
  if (props.progress.status === 'succeeded') return 100;
  const stage = props.progress.current;
  return props.progress.status === 'running' && stage?.total
    ? Math.min(100, (stage.completed / stage.total) * 100)
    : undefined;
});
const labels = {
  not_started: 'Не началась',
  running: 'Выполняется',
  succeeded: 'Завершена',
  failed: 'Ошибка',
  cancelled: 'Отменена',
  interrupted: 'Прервана',
};
</script>

<template>
  <section class="phase" aria-labelledby="conversion-title">
    <div class="phase-heading">
      <h3 id="conversion-title">Конвертация</h3>
      <span class="status-text">{{ labels[progress.status] }}</span>
    </div>
    <div
      class="meter"
      :class="{
        'is-indeterminate': progress.status === 'running' && percent === undefined,
        'is-complete': progress.status === 'succeeded',
      }"
      role="progressbar"
      aria-label="Конвертация"
      aria-valuemin="0"
      aria-valuemax="100"
      :aria-valuenow="percent"
      :aria-valuetext="
        progress.status === 'running' && progress.current
          ? `Конвертация: ${stageLabels[progress.current.stage].toLocaleLowerCase('ru')}`
          : labels[progress.status]
      "
      :style="{ '--fill': `${percent ?? 0}%` }"
    >
      <span />
    </div>
    <p v-if="progress.status === 'not_started'" class="muted">Ожидает скачивания и подготовки</p>
    <template v-else-if="progress.current">
      <div class="counter-line">
        <strong>{{ stageLabels[progress.current.stage] }}</strong
        ><span>{{ stageCounter(progress.current) }}</span>
      </div>
      <p class="muted">
        Статей: {{ formatCount(progress.current.articles) }} · Ресурсов:
        {{ formatCount(progress.current.resources) }}
      </p>
    </template>
    <ul
      v-if="progress.completed_stages.length"
      class="completed-stages"
      aria-label="Завершённые этапы"
    >
      <li v-for="stage in progress.completed_stages" :key="stage.stage">
        {{ stageLabels[stage.stage] }} <span>— завершено</span>
      </li>
    </ul>
    <p v-if="progress.status === 'interrupted'" class="notice-text">
      Конвертация прервана при закрытии. Запустите её заново; скачанные файлы сохранены.
    </p>
  </section>
</template>
