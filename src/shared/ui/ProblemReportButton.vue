<script setup lang="ts">
import { ref } from 'vue';
import Button from 'primevue/button';
import Icon from './Icon.vue';
import type { NativeBridge } from '../contracts';
import { errorMessage } from '../format';

const props = defineProps<{ bridge: NativeBridge; jobId?: string; disabled?: boolean }>();
const emit = defineEmits<{ error: [message: string] }>();
const opening = ref(false);
async function report() {
  if (opening.value || props.disabled) return;
  opening.value = true;
  emit('error', '');
  try {
    await props.bridge.openIssue(props.jobId);
  } catch (error) {
    emit('error', errorMessage(error));
  } finally {
    opening.value = false;
  }
}
</script>
<template>
  <Button
    class="icon-button issue-button"
    text
    severity="secondary"
    aria-label="Сообщить о проблеме"
    title="Сообщить о проблеме"
    :disabled="disabled || opening"
    :aria-busy="opening"
    @click="report"
    ><Icon name="mail"
  /></Button>
</template>
