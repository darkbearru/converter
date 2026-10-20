<script setup lang="ts">
import { displayPath } from '../../shared/format';
import { manualEncodings } from '../../shared/encodings';
import { errorState } from '../../shared/i18n/errors';

import { useI18n } from 'vue-i18n';
const { t } = useI18n();

import { ref, useId } from 'vue';
import type {
  JobOptionsUpdate,
  JobSnapshot,
  NativeBridge,
  OutputChoice,
} from '../../shared/contracts';
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
const error = errorState();
async function pickOutput() {
  try {
    output.value = (await props.bridge.pickOutput()) ?? undefined;
  } catch (e) {
    error.value = e;
  }
}
async function pickResources() {
  try {
    const files = await props.bridge.pickSources('resources');
    resourceIds.value = files.map((f) => f.import_id);
    names.value = files.map((f) => f.display_name);
  } catch (e) {
    error.value = e;
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
    <h3>{{ t('app.review_title') }}</h3>
    <p class="muted">{{ t('app.review_hint') }}</p>
    <div v-if="job.input_choices.length" class="field-stack">
      <label :for="`${id}-dictionary`"> {{ t('app.dictionary_in_archive') }} </label
      ><select :id="`${id}-dictionary`" v-model="dictionary">
        <option v-for="choice in job.input_choices" :key="choice.id" :value="choice.id">
          {{ choice.display_name }}
        </option>
        <option value="*">{{ t('app.each_dictionary') }}</option>
      </select>
    </div>
    <label :for="`${id}-encoding`"> {{ t('app.encoding') }} </label
    ><select :id="`${id}-encoding`" v-model="encoding">
      <option value="auto">{{ t('app.auto_encoding') }}</option>
      <option v-for="item in manualEncodings" :key="item.value" :value="item.value">
        {{ item.label }}
      </option>
    </select>
    <div class="button-row">
      <button type="button" @click="pickOutput">{{ t('app.change_output') }}</button
      ><button type="button" @click="pickResources">{{ t('app.replace_resources') }}</button>
    </div>
    <p v-if="output" class="hint destination-path">
      {{ output === 'app_library' ? t('app.app_library') : displayPath(output.desktop_folder) }}
    </p>
    <p v-if="names.length" class="hint">{{ names.join(', ') }}</p>
    <p v-if="error" role="alert" class="error-message">{{ error }}</p>
    <button type="submit" class="primary" :disabled="busy">{{ t('app.save_start') }}</button>
  </form>
</template>
