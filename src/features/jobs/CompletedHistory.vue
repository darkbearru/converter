<script setup lang="ts">
import { reactive, ref, watch } from 'vue';
import Accordion from 'primevue/accordion';
import AccordionPanel from 'primevue/accordionpanel';
import AccordionHeader from 'primevue/accordionheader';
import AccordionContent from 'primevue/accordioncontent';
import Button from 'primevue/button';
import Icon from '../../shared/ui/Icon.vue';
import ResultPanel from '../results/ResultPanel.vue';
import DiagnosticsList from '../results/DiagnosticsList.vue';
import type { JobSnapshot, NativeBridge } from '../../shared/contracts';
import { errorMessage, stateLabels } from '../../shared/format';
const props = defineProps<{
  jobs: JobSnapshot[];
  bridge: NativeBridge;
  pending: Record<string, boolean>;
}>();
defineEmits<{
  open: [id: string];
  export: [id: string];
  retry: [id: string];
  repair: [id: string];
}>();
const expanded = ref<string[]>([]);
const paths = reactive<Record<string, string>>({});
const pathErrors = reactive<Record<string, string>>({});
const resultIds = new Map<string, string>();
watch(
  () =>
    expanded.value
      .map((id) => `${id}:${props.jobs.find((job) => job.id === id)?.result?.id ?? ''}`)
      .join('|'),
  async () => {
    await Promise.all(
      expanded.value.map(async (id) => {
        const job = props.jobs.find((item) => item.id === id);
        const resultId = job?.result?.id;
        if (resultIds.get(id) === resultId) return;
        delete paths[id];
        delete pathErrors[id];
        if (!resultId) {
          resultIds.delete(id);
          return;
        }
        resultIds.set(id, resultId);
        try {
          const path = await props.bridge.getResultPath(id);
          if (resultIds.get(id) === resultId) paths[id] = path;
        } catch (e) {
          if (resultIds.get(id) === resultId) {
            pathErrors[id] = errorMessage(e);
            resultIds.delete(id);
          }
        }
      }),
    );
  },
);
function warning(job: JobSnapshot) {
  return (
    job.had_errors ||
    job.diagnostic_count > 0 ||
    job.state === 'succeeded_with_warnings' ||
    ['failed', 'interrupted', 'awaiting_input'].includes(job.state)
  );
}
</script>
<template>
  <section class="completed-history" aria-label="История конвертаций">
    <Accordion v-model:value="expanded" multiple>
      <AccordionPanel v-for="job in jobs" :key="job.id" :value="job.id">
        <AccordionHeader
          ><span class="history-file" :title="job.display_name">{{ job.display_name }}</span
          ><Icon
            v-if="warning(job)"
            name="caution"
            label="Были предупреждения или ошибки"
            class="history-warning" /><template #toggleicon
            ><Icon
              name="chevron"
              class="history-chevron"
              :class="{ 'is-open': expanded.includes(job.id) }" /></template
        ></AccordionHeader>
        <AccordionContent>
          <p class="history-status">{{ stateLabels[job.state] }}</p>
          <p v-if="job.error" class="error-message" role="alert">{{ job.error.message }}</p>
          <ResultPanel
            v-if="job.result"
            :result="job.result"
            :export-state="job.export"
            :busy="Boolean(pending[job.id])"
            @open="$emit('open', job.id)"
            @export="$emit('export', job.id)"
          />
          <Button
            v-if="paths[job.id]"
            class="result-path"
            text
            :label="paths[job.id]"
            :aria-label="`Открыть папку ${paths[job.id]}`"
            @click="$emit('open', job.id)"
            ><template #icon><Icon name="folder" /></template
          ></Button>
          <p v-if="pathErrors[job.id]" class="error-message" role="alert">
            {{ pathErrors[job.id] }}
          </p>
          <Button
            v-if="job.state === 'awaiting_input'"
            label="Уточнить параметры"
            outlined
            @click="$emit('repair', job.id)"
          />
          <Button
            v-if="['cancelled', 'failed', 'interrupted'].includes(job.state)"
            label="Запустить заново"
            outlined
            :disabled="pending[job.id]"
            @click="$emit('retry', job.id)"
            ><template #icon><Icon name="refresh" /></template
          ></Button>
          <Button
            v-if="['succeeded', 'succeeded_with_warnings'].includes(job.state)"
            label="Повторить конвертацию"
            outlined
            :disabled="pending[job.id]"
            @click="$emit('retry', job.id)"
            ><template #icon><Icon name="refresh" /></template
          ></Button>
          <DiagnosticsList
            v-if="job.diagnostic_count"
            :id="job.id"
            :count="job.diagnostic_count"
            :bridge="bridge"
          />
        </AccordionContent>
      </AccordionPanel>
    </Accordion>
  </section>
</template>
