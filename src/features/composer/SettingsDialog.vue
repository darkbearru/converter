<script setup lang="ts">
import { errorState } from '../../shared/i18n/errors';

import { useI18n } from 'vue-i18n';
const { t } = useI18n();

import { computed, useId } from 'vue';
import Dialog from 'primevue/dialog';
import { dialogCloseButtonProps } from '../../shared/ui/dialog';
import Select from 'primevue/select';
import Button from 'primevue/button';
import InputText from 'primevue/inputtext';
import Textarea from 'primevue/textarea';
import Icon from '../../shared/ui/Icon.vue';
import type { NativeBridge } from '../../shared/contracts';
import type { ComposerSettings } from './model';
const props = defineProps<{
  visible: boolean;
  settings: ComposerSettings;
  bridge: NativeBridge;
  resourcesLocked?: boolean;
}>();
const emit = defineEmits<{ 'update:visible': [value: boolean]; resourcesChange: [] }>();
const id = useId();
const error = errorState();
const encodings = computed(() => [
  { label: t('app.auto_encoding'), value: 'auto' },
  { label: 'UTF-8', value: 'utf-8' },
  { label: 'UTF-16 LE', value: 'utf-16le' },
  { label: 'UTF-16 BE', value: 'utf-16be' },
  { label: 'Windows-1251', value: 'windows-1251' },
]);
async function output() {
  error.value = '';
  try {
    const value = await props.bridge.pickOutput();
    if (value) props.settings.output = value;
  } catch (e) {
    error.value = e;
  }
}
async function resources(purpose: 'resources' | 'resource_directory') {
  error.value = '';
  try {
    const selected = await props.bridge.pickSources(purpose);
    if (selected.length) {
      props.settings.resources.push(...selected);
      emit('resourcesChange');
    }
  } catch (e) {
    error.value = e;
  }
}
</script>
<template>
  <Dialog
    :close-button-props="dialogCloseButtonProps"
    :visible="visible"
    modal
    :header="t('app.conversion_settings')"
    class="settings-dialog app-dialog"
    @update:visible="emit('update:visible', $event)"
  >
    <template #closeicon><Icon name="close" /></template>
    <div class="dialog-fields" tabindex="-1" autofocus>
      <div class="field-stack">
        <label :id="`${id}-encoding-label`" :for="`${id}-encoding`"> {{ t('app.encoding') }} </label
        ><Select
          :input-id="`${id}-encoding`"
          :aria-labelledby="`${id}-encoding-label`"
          v-model="settings.encoding"
          :options="encodings"
          option-label="label"
          option-value="value"
          ><template #dropdownicon><Icon name="chevron" /></template
        ></Select>
      </div>
      <div class="field-stack">
        <label> {{ t('app.save_to') }} </label>
        <p class="destination-path">
          {{
            settings.output === 'app_library'
              ? t('app.app_library')
              : settings.output.desktop_folder
          }}
        </p>
        <div class="button-row">
          <Button :label="t('app.choose_folder')" outlined @click="output" /><Button
            v-if="settings.output !== 'app_library'"
            :label="t('app.to_library')"
            text
            @click="settings.output = 'app_library'"
          />
        </div>
      </div>
      <fieldset class="resource-settings" :disabled="resourcesLocked">
        <legend>{{ t('app.images_audio') }}</legend>
        <div class="button-row">
          <Button
            :label="t('app.add_resources')"
            outlined
            :disabled="resourcesLocked"
            @click="resources('resources')"
          /><Button
            :label="t('app.add_resource_folder')"
            outlined
            :disabled="resourcesLocked"
            @click="resources('resource_directory')"
          />
        </div>
        <ul v-if="settings.resources.length" class="attachment-list">
          <li v-for="(file, index) in settings.resources" :key="file.import_id">
            <span>{{ file.display_name }}</span
            ><Button
              :aria-label="t('app.remove_named', { name: file.display_name })"
              text
              rounded
              :disabled="resourcesLocked"
              @click="
                settings.resources.splice(index, 1);
                emit('resourcesChange');
              "
              ><Icon name="close"
            /></Button>
          </li>
        </ul>
        <p v-if="resourcesLocked" class="hint">{{ t('app.resources_locked') }}</p>
        <ul v-if="settings.retainedResources.length" class="attachment-list">
          <li v-for="(file, index) in settings.retainedResources" :key="file.source_id">
            <span>{{ file.display_name }}</span>
            <Button
              :aria-label="t('app.remove_named', { name: file.display_name })"
              text
              rounded
              :disabled="resourcesLocked"
              @click="
                settings.retainedResources.splice(index, 1);
                emit('resourcesChange');
              "
              ><Icon name="close"
            /></Button>
          </li>
        </ul>
      </fieldset>
      <div class="field-stack">
        <label :for="`${id}-resources`"> {{ t('app.resource_urls') }} </label
        ><Textarea
          :id="`${id}-resources`"
          v-model="settings.resourceUrls"
          @update:model-value="emit('resourcesChange')"
          :disabled="resourcesLocked"
          rows="2"
          :placeholder="t('app.urls_placeholder')"
        />
      </div>
      <div class="field-stack">
        <label :for="`${id}-title`"> {{ t('app.dictionary_title') }} </label
        ><InputText
          :id="`${id}-title`"
          v-model="settings.title"
          :placeholder="t('app.from_source')"
        />
      </div>
      <div class="field-stack">
        <label :for="`${id}-description`"> {{ t('app.description') }} </label
        ><Textarea :id="`${id}-description`" v-model="settings.description" rows="2" />
      </div>
      <p v-if="error" role="alert" class="error-message">{{ error }}</p>
    </div>
    <template #footer
      ><Button :label="t('app.done')" @click="emit('update:visible', false)"
    /></template>
  </Dialog>
</template>
