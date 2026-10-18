<script setup lang="ts">
import { nextTick, ref, watch } from 'vue';
import ProgressBar from 'primevue/progressbar';
import Button from 'primevue/button';
import Icon from '../../shared/ui/Icon.vue';
import type { JobSnapshot } from '../../shared/contracts';
import { stageLabels, stateLabels } from '../../shared/format';
const props = defineProps<{ jobs: JobSnapshot[]; pending: Record<string, boolean> }>();
const stack = ref<{ $el: HTMLElement }>();
watch(
  () => props.jobs.length,
  async (count, previous) => {
    if (count <= previous) return;
    await nextTick();
    const element = stack.value?.$el;
    if (element) element.scrollTop = element.scrollHeight;
  },
);
defineEmits<{
  cancel: [id: string];
  pause: [id: string];
  resume: [id: string];
  details: [id: string];
}>();
function percent(job: JobSnapshot) {
  if (
    ['downloading', 'download_retry_wait', 'download_paused', 'download_failed'].includes(job.state)
  )
    return job.download.percent;
  const stage = job.conversion.current;
  return stage?.total ? Math.min(100, (100 * stage.completed) / stage.total) : null;
}
function label(job: JobSnapshot) {
  if (
    ['downloading', 'download_retry_wait', 'download_paused', 'download_failed'].includes(job.state)
  )
    return stateLabels[job.state];
  return job.conversion.current
    ? stageLabels[job.conversion.current.stage]
    : stateLabels[job.state];
}
</script>
<template>
  <TransitionGroup
    ref="stack"
    name="task"
    tag="section"
    class="task-stack"
    aria-label="Активные задачи"
  >
    <article
      v-for="(job, index) in jobs"
      :key="job.id"
      class="active-task"
      :style="{ '--stack-inset': `${Math.min(jobs.length - index - 1, 3) * 8}px` }"
    >
      <Button
        class="task-name"
        text
        :label="job.display_name"
        :title="`${job.display_name} — ${label(job)}`"
        @click="$emit('details', job.id)"
      />
      <div class="task-progress">
        <ProgressBar
          :aria-label="`${job.display_name}: ${label(job)}`"
          :mode="percent(job) === null ? 'indeterminate' : 'determinate'"
          :value="percent(job) ?? undefined"
          :show-value="false"
        /><span class="sr-only" role="status"
          >{{ label(job)
          }}<template v-if="percent(job) !== null">
            {{ Math.round(percent(job)!) }}%</template
          ></span
        >
      </div>
      <div class="task-controls">
        <Button
          class="icon-button"
          :aria-label="`Отменить ${job.display_name}`"
          title="Отменить задачу"
          text
          rounded
          :disabled="pending[job.id]"
          @click="$emit('cancel', job.id)"
          ><Icon name="remove" /></Button
        ><Button
          v-if="['downloading', 'download_retry_wait'].includes(job.state)"
          class="icon-button"
          :aria-label="`Приостановить скачивание ${job.display_name}`"
          text
          rounded
          :disabled="pending[job.id]"
          @click="$emit('pause', job.id)"
          ><Icon name="pause" /></Button
        ><Button
          v-if="['download_paused', 'download_failed'].includes(job.state)"
          class="icon-button"
          :aria-label="`Продолжить скачивание ${job.display_name}`"
          text
          rounded
          :disabled="pending[job.id]"
          @click="$emit('resume', job.id)"
          ><Icon name="play"
        /></Button>
      </div>
    </article>
  </TransitionGroup>
</template>
