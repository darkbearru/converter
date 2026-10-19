<script setup lang="ts">
import { ref, useId, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import Dialog from 'primevue/dialog';
import Button from 'primevue/button';
import InputText from 'primevue/inputtext';
import Message from 'primevue/message';
import Icon from '../../shared/ui/Icon.vue';
import { dialogCloseButtonProps } from '../../shared/ui/dialog';
import { errorMessage } from '../../shared/format';
import type { JobSnapshot } from '../../shared/contracts';
const props = defineProps<{ job: JobSnapshot | null; busy: boolean; error: unknown }>();
const emit = defineEmits<{ submit: [password: string]; close: [] }>();
const { t } = useI18n();
const id = useId();
const password = ref('');
watch(
  () => [props.job?.id, props.job?.generation],
  () => {
    password.value = '';
  },
);
function close() {
  password.value = '';
  emit('close');
}
function submit() {
  if (props.busy || !password.value) return;
  const value = password.value;
  password.value = '';
  emit('submit', value);
}
</script>
<template>
  <Dialog
    :visible="Boolean(job)"
    modal
    :header="t('app.archive_password_title')"
    class="app-dialog"
    :close-button-props="dialogCloseButtonProps"
    @update:visible="!$event && close()"
  >
    <template #closeicon><Icon name="close" /></template>
    <form :id="id" class="dialog-fields" @submit.prevent="submit">
      <Message
        v-if="job?.error"
        :severity="job.error.code === 'archive_password_invalid' ? 'error' : 'info'"
        >{{ errorMessage(job.error) }}</Message
      >
      <div class="field-stack">
        <label :for="`${id}-password`">{{ t('app.password') }}</label>
        <InputText
          :id="`${id}-password`"
          v-model="password"
          type="password"
          autocomplete="off"
          :spellcheck="false"
          :disabled="busy"
          autofocus
        />
      </div>
      <Message v-if="error" severity="error">{{ errorMessage(error) }}</Message>
    </form>
    <template #footer>
      <Button
        :label="t('app.cancel')"
        severity="secondary"
        outlined
        :disabled="busy"
        @click="close"
      />
      <Button
        :label="t('app.continue')"
        severity="secondary"
        outlined
        type="submit"
        :form="id"
        :disabled="busy || !password"
      />
    </template>
  </Dialog>
</template>
