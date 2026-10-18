<script setup lang="ts">
import { ref, useId } from 'vue';
import type {
  JobOptionsUpdate,
  JobSnapshot,
  NativeBridge,
  OutputChoice,
} from '../../shared/contracts';
import { errorMessage } from '../../shared/format';
const props = defineProps<{ job: JobSnapshot; bridge: NativeBridge; busy: boolean }>();
defineEmits<{ save: [update: JobOptionsUpdate] }>();
const id = useId();
const encoding = ref(
  typeof props.job.options.encoding === 'string' ? 'auto' : props.job.options.encoding.manual,
);
const dictionary = ref(props.job.input_choices[0]?.id ?? '');
const output = ref<OutputChoice | undefined>();
const resourceIds = ref<string[] | undefined>();
const names = ref<string[]>([]);
const error = ref('');
async function pickOutput() {
  try {
    output.value = (await props.bridge.pickOutput()) ?? undefined;
  } catch (e) {
    error.value = errorMessage(e);
  }
}
async function pickResources() {
  try {
    const files = await props.bridge.pickSources('resources');
    resourceIds.value = files.map((f) => f.import_id);
    names.value = files.map((f) => f.display_name);
  } catch (e) {
    error.value = errorMessage(e);
  }
}
</script>
<template>
  <form
    class="job-settings"
    @submit.prevent="
      $emit('save', {
        options: { encoding: encoding === 'auto' ? 'auto' : { manual: encoding } },
        dictionary_id: dictionary || undefined,
        output,
        resources: resourceIds,
      })
    "
  >
    <h3>Уточните параметры</h3>
    <p class="muted">Исправьте причину остановки и продолжите задание.</p>
    <div v-if="job.input_choices.length" class="field-stack">
      <label :for="`${id}-dictionary`">Словарь в архиве</label
      ><select :id="`${id}-dictionary`" v-model="dictionary">
        <option v-for="choice in job.input_choices" :key="choice.id" :value="choice.id">
          {{ choice.display_name }}
        </option>
        <option value="*">Каждый словарь отдельным заданием</option>
      </select>
    </div>
    <label :for="`${id}-encoding`">Кодировка DSL</label
    ><select :id="`${id}-encoding`" v-model="encoding">
      <option value="auto">Определить автоматически</option>
      <option value="utf-8">UTF-8</option>
      <option value="utf-16le">UTF-16 LE</option>
      <option value="utf-16be">UTF-16 BE</option>
      <option value="windows-1251">Windows-1251</option>
    </select>
    <div class="button-row">
      <button type="button" @click="pickOutput">Изменить папку результата</button
      ><button type="button" @click="pickResources">Заменить набор ресурсов</button>
    </div>
    <p v-if="output" class="hint destination-path">
      {{ output === 'app_library' ? 'Библиотека приложения' : output.desktop_folder }}
    </p>
    <p v-if="names.length" class="hint">{{ names.join(', ') }}</p>
    <p v-if="error" role="alert" class="error-message">{{ error }}</p>
    <button type="submit" class="primary" :disabled="busy">Сохранить параметры и запустить</button>
  </form>
</template>
