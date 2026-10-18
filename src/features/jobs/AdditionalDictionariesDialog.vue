<script setup lang="ts">
import Dialog from 'primevue/dialog';
import Button from 'primevue/button';
import Icon from '../../shared/ui/Icon.vue';
import type { AdditionalDictionariesOffer } from '../../shared/contracts';
const props = defineProps<{
  offer?: AdditionalDictionariesOffer | null;
  busy: boolean;
  error: string;
}>();
const emit = defineEmits<{ decide: [convert: boolean]; dismiss: [] }>();
function close() {
  if (props.busy) return;
  if (props.error) emit('dismiss');
  else emit('decide', false);
}
</script>
<template>
  <Dialog
    :visible="Boolean(offer)"
    modal
    header="Другие словари в архиве"
    class="app-dialog additional-dictionaries-dialog"
    :closable="!busy"
    :close-on-escape="!busy"
    @update:visible="!$event && close()"
  >
    <template #closeicon><Icon name="close" /></template>
    <div class="dialog-fields">
      <p>
        Основной словарь — <strong>{{ offer?.main_name }}</strong
        >.
      </p>
      <p>Найдены независимые словари. Сконвертировать их отдельными заданиями?</p>
      <ul class="additional-dictionary-list">
        <li v-for="dictionary in offer?.dictionaries" :key="dictionary.id">
          {{ dictionary.display_name }}
        </li>
      </ul>
      <p v-if="busy" role="status" class="muted">Сохранение выбора…</p>
      <p v-if="error" role="alert" class="error-message">{{ error }}</p>
    </div>
    <template #footer>
      <Button
        :label="error ? 'Закрыть' : 'Пропустить'"
        severity="secondary"
        outlined
        autofocus
        :disabled="busy"
        @click="close"
      />
      <Button label="Конвертировать" :disabled="busy" @click="emit('decide', true)" />
    </template>
  </Dialog>
</template>
