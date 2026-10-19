<script setup lang="ts">
import { useI18n } from 'vue-i18n';
const { t } = useI18n();

import { ref } from 'vue';
import Button from 'primevue/button';
import Icon from './Icon.vue';
import type { NativeBridge } from '../contracts';

const props = defineProps<{
  bridge: NativeBridge;
  jobId?: string;
  disabled?: boolean;
  outlined?: boolean;
}>();
const emit = defineEmits<{ error: [message: unknown] }>();
const opening = ref(false);
async function report() {
  if (opening.value || props.disabled) return;
  opening.value = true;
  emit('error', '');
  try {
    await props.bridge.openIssue(props.jobId);
  } catch (error) {
    emit('error', error);
  } finally {
    opening.value = false;
  }
}
</script>
<template>
  <Button
    class="issue-button"
    :class="{ 'icon-button': !outlined }"
    :text="!outlined"
    :outlined="outlined"
    severity="secondary"
    :aria-label="t('app.report_issue')"
    :title="t('app.report_issue')"
    :disabled="disabled || opening"
    :aria-busy="opening"
    @click="report"
    ><Icon name="mail"
  /></Button>
</template>
