<script setup lang="ts">
import { errorMessage } from '../../shared/format';
import { useI18n } from 'vue-i18n';
const { t } = useI18n();

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
import { stateLabel } from '../../shared/format';
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
const reportErrors = ref<Record<string, unknown>>({});
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
  <section class="completed-history" :aria-label="t('app.history')">
    <Accordion v-model:value="expanded" multiple>
      <AccordionPanel v-for="job in jobs" :key="job.id" :value="job.id">
        <AccordionHeader
          ><span class="history-file" :title="job.display_name">{{ job.display_name }}</span
          ><Icon
            v-if="warning(job)"
            name="caution"
            :label="t('app.had_warnings')"
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
            {{ stateLabel(job.state) }}
          </p>
          <ResultPanel v-if="job.result" :result="job.result" :export-state="job.export" />
          <Message v-if="job.error" severity="error">{{ errorMessage(job.error) }}</Message>
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
          <div class="history-actions" role="group" :aria-label="t('app.history_actions')">
            <Button
              v-if="job.result?.can_open"
              :label="t('app.open_folder')"
              severity="secondary"
              outlined
              :disabled="pending[job.id]"
              @click="$emit('open', job.id)"
            />
            <Button
              v-if="job.result?.can_export"
              :label="job.export.status === 'running' ? t('app.saving') : t('app.save_copy')"
              severity="secondary"
              outlined
              :disabled="pending[job.id] || job.export.status === 'running'"
              @click="$emit('export', job.id)"
            />
            <Button
              v-if="job.state === 'awaiting_input'"
              :label="t('app.review_settings')"
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
              :label="t('app.retry')"
              severity="secondary"
              outlined
              :disabled="pending[job.id] || job.export.status === 'running'"
              @click="$emit('retry', job.id)"
            />
            <Button
              :label="t('app.delete')"
              severity="danger"
              outlined
              :disabled="pending[job.id] || job.export.status === 'running'"
              @click="
                focusDialogOpener($event);
                $emit('remove', job.id);
              "
            />
            <ProblemReportButton
              outlined
              :bridge="bridge"
              :job-id="job.id"
              :disabled="pending[job.id]"
              @error="reportErrors[job.id] = $event"
            />
          </div>
          <Message v-if="reportErrors[job.id]" severity="error" role="alert">{{
            errorMessage(reportErrors[job.id])
          }}</Message>
        </AccordionContent>
      </AccordionPanel>
    </Accordion>
    <Dialog
      :close-button-props="dialogCloseButtonProps"
      :id="diagnosticsJob ? `diagnostics-${diagnosticsJob.id}` : undefined"
      :visible="Boolean(diagnosticsJob)"
      modal
      :header="t('app.warnings')"
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
        <Button
          :label="t('app.close')"
          severity="secondary"
          outlined
          @click="diagnosticsId = null"
        />
      </template>
    </Dialog>
  </section>
</template>
