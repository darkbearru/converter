<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import Accordion from 'primevue/accordion';
import AccordionPanel from 'primevue/accordionpanel';
import AccordionHeader from 'primevue/accordionheader';
import AccordionContent from 'primevue/accordioncontent';
import Button from 'primevue/button';
import Dialog from 'primevue/dialog';
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
  remove: [id: string];
}>();
const expanded = ref<string[]>([]);
const diagnosticsId = ref<string | null>(null);
const diagnosticsJob = computed(() => props.jobs.find((job) => job.id === diagnosticsId.value));
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
          <DiagnosticsList
            v-if="job.diagnostic_count && expanded.includes(job.id)"
            :key="`${job.id}:${job.run_id}`"
            :id="job.id"
            :count="job.diagnostic_count"
            :bridge="bridge"
            preview
            @show-all="diagnosticsId = job.id"
          />
          <ResultPanel v-if="job.result" :result="job.result" :export-state="job.export" />
          <div class="history-actions" role="group" aria-label="Действия с конвертацией">
            <Button
              v-if="job.result?.can_open"
              label="Открыть папку"
              severity="secondary"
              outlined
              :disabled="pending[job.id]"
              @click="$emit('open', job.id)"
            />
            <Button
              v-if="job.result?.can_export"
              :label="job.export.status === 'running' ? 'Сохранение…' : 'Сохранить копию'"
              severity="secondary"
              outlined
              :disabled="pending[job.id] || job.export.status === 'running'"
              @click="$emit('export', job.id)"
            />
            <Button
              v-if="job.state === 'awaiting_input'"
              label="Уточнить параметры"
              severity="secondary"
              outlined
              :disabled="pending[job.id]"
              @click="$emit('repair', job.id)"
            />
            <Button
              v-else
              label="Повторить"
              severity="secondary"
              outlined
              :disabled="pending[job.id] || job.export.status === 'running'"
              @click="$emit('retry', job.id)"
            />
            <Button
              label="Удалить"
              severity="danger"
              outlined
              :disabled="pending[job.id] || job.export.status === 'running'"
              @click="$emit('remove', job.id)"
            />
          </div>
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
        </AccordionContent>
      </AccordionPanel>
    </Accordion>
    <Dialog
      :id="diagnosticsJob ? `diagnostics-${diagnosticsJob.id}` : undefined"
      :visible="Boolean(diagnosticsJob)"
      modal
      header="Предупреждения"
      class="app-dialog diagnostics-dialog"
      @update:visible="!$event && (diagnosticsId = null)"
    >
      <template #closeicon><Icon name="close" /></template>
      <template v-if="diagnosticsJob">
        <p class="diagnostics-filename muted" :title="diagnosticsJob.display_name">
          {{ diagnosticsJob.display_name }}
        </p>
        <DiagnosticsList
          :key="`${diagnosticsJob.id}:${diagnosticsJob.run_id}`"
          :id="diagnosticsJob.id"
          :count="diagnosticsJob.diagnostic_count"
          :bridge="bridge"
          expanded
        />
      </template>
      <template #footer>
        <Button label="Закрыть" severity="secondary" outlined @click="diagnosticsId = null" />
      </template>
    </Dialog>
  </section>
</template>
