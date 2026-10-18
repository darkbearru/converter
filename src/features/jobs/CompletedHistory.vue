<script setup lang="ts">
import { computed, ref } from 'vue';
import Accordion from 'primevue/accordion';
import AccordionPanel from 'primevue/accordionpanel';
import AccordionHeader from 'primevue/accordionheader';
import AccordionContent from 'primevue/accordioncontent';
import Button from 'primevue/button';
import Dialog from 'primevue/dialog';
import Message from 'primevue/message';
import { dialogCloseButtonProps, focusDialogOpener } from '../../shared/ui/dialog';
import Icon from '../../shared/ui/Icon.vue';
import ProblemReportButton from '../../shared/ui/ProblemReportButton.vue';
import ResultPanel from '../results/ResultPanel.vue';
import DiagnosticsList from '../results/DiagnosticsList.vue';
import type { JobSnapshot, NativeBridge } from '../../shared/contracts';
import { stateLabels } from '../../shared/format';
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
const reportErrors = ref<Record<string, string>>({});
const diagnosticsJob = computed(() => props.jobs.find((job) => job.id === diagnosticsId.value));
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
          <p
            v-if="!['succeeded', 'succeeded_with_warnings'].includes(job.state)"
            class="history-status"
          >
            {{ stateLabels[job.state] }}
          </p>
          <ResultPanel v-if="job.result" :result="job.result" :export-state="job.export" />
          <Message v-if="job.error" severity="error">{{ job.error.message }}</Message>
          <DiagnosticsList
            v-if="job.diagnostic_count && expanded.includes(job.id)"
            :key="`${job.id}:${job.run_id}`"
            :id="job.id"
            :count="job.diagnostic_count"
            :bridge="bridge"
            :severity="
              job.error || ['failed', 'interrupted'].includes(job.state) ? 'error' : 'warn'
            "
            preview
            @show-all="diagnosticsId = job.id"
          />
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
              @click="
                focusDialogOpener($event);
                $emit('repair', job.id);
              "
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
              @click="
                focusDialogOpener($event);
                $emit('remove', job.id);
              "
            />
            <ProblemReportButton
              :bridge="bridge"
              :job-id="job.id"
              :disabled="pending[job.id]"
              @error="reportErrors[job.id] = $event"
            />
          </div>
          <Message v-if="reportErrors[job.id]" severity="error" role="alert">{{
            reportErrors[job.id]
          }}</Message>
        </AccordionContent>
      </AccordionPanel>
    </Accordion>
    <Dialog
      :close-button-props="dialogCloseButtonProps"
      :id="diagnosticsJob ? `diagnostics-${diagnosticsJob.id}` : undefined"
      :visible="Boolean(diagnosticsJob)"
      modal
      header="Предупреждения"
      class="app-dialog diagnostics-dialog"
      @update:visible="!$event && (diagnosticsId = null)"
    >
      <template #closeicon><Icon name="close" /></template>
      <template v-if="diagnosticsJob">
        <p
          class="diagnostics-filename muted"
          tabindex="-1"
          autofocus
          :title="diagnosticsJob.display_name"
        >
          {{ diagnosticsJob.display_name }}
        </p>
        <DiagnosticsList
          :key="`${diagnosticsJob.id}:${diagnosticsJob.run_id}`"
          :id="diagnosticsJob.id"
          :count="diagnosticsJob.diagnostic_count"
          :bridge="bridge"
          :severity="
            diagnosticsJob.error || ['failed', 'interrupted'].includes(diagnosticsJob.state)
              ? 'error'
              : 'warn'
          "
          expanded
        />
      </template>
      <template #footer>
        <Button label="Закрыть" severity="secondary" outlined @click="diagnosticsId = null" />
      </template>
    </Dialog>
  </section>
</template>
