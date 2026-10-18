<script setup lang="ts">
import { ref, useId } from 'vue';
import Dialog from 'primevue/dialog';
import Select from 'primevue/select';
import Button from 'primevue/button';
import InputText from 'primevue/inputtext';
import Textarea from 'primevue/textarea';
import Icon from '../../shared/ui/Icon.vue';
import type { NativeBridge } from '../../shared/contracts';
import { errorMessage } from '../../shared/format';
import type { ComposerSettings } from './model';
const props = defineProps<{
  visible: boolean;
  settings: ComposerSettings;
  bridge: NativeBridge;
  resourcesLocked?: boolean;
}>();
const emit = defineEmits<{ 'update:visible': [value: boolean]; resourcesChange: [] }>();
const id = useId();
const error = ref('');
const encodings = [
  { label: 'Определить автоматически', value: 'auto' },
  { label: 'UTF-8', value: 'utf-8' },
  { label: 'UTF-16 LE', value: 'utf-16le' },
  { label: 'UTF-16 BE', value: 'utf-16be' },
  { label: 'Windows-1251', value: 'windows-1251' },
];
async function output() {
  error.value = '';
  try {
    const value = await props.bridge.pickOutput();
    if (value) props.settings.output = value;
  } catch (e) {
    error.value = errorMessage(e);
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
    error.value = errorMessage(e);
  }
}
</script>
<template>
  <Dialog
    :visible="visible"
    modal
    header="Настройки конвертации"
    class="settings-dialog app-dialog"
    @update:visible="emit('update:visible', $event)"
  >
    <template #closeicon><Icon name="close" /></template>
    <div class="dialog-fields">
      <div class="field-stack">
        <label :id="`${id}-encoding-label`" :for="`${id}-encoding`">Кодировка DSL</label
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
        <label>Куда сохранить</label>
        <p class="destination-path">
          {{
            settings.output === 'app_library'
              ? 'Библиотека приложения'
              : settings.output.desktop_folder
          }}
        </p>
        <div class="button-row">
          <Button label="Выбрать папку" outlined @click="output" /><Button
            v-if="settings.output !== 'app_library'"
            label="В библиотеку"
            text
            @click="settings.output = 'app_library'"
          />
        </div>
      </div>
      <fieldset class="resource-settings" :disabled="resourcesLocked">
        <legend>Изображения и озвучка</legend>
        <div class="button-row">
          <Button
            label="Добавить файлы ресурсов"
            outlined
            :disabled="resourcesLocked"
            @click="resources('resources')"
          /><Button
            label="Добавить папку ресурсов"
            outlined
            :disabled="resourcesLocked"
            @click="resources('resource_directory')"
          />
        </div>
        <ul v-if="settings.resources.length" class="attachment-list">
          <li v-for="(file, index) in settings.resources" :key="file.import_id">
            <span>{{ file.display_name }}</span
            ><Button
              :aria-label="`Убрать ${file.display_name}`"
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
        <p v-if="resourcesLocked" class="hint">
          Ресурсы этого файла уже выбраны перед скачиванием.
        </p>
        <ul v-if="settings.retainedResources.length" class="attachment-list">
          <li v-for="(file, index) in settings.retainedResources" :key="file.source_id">
            <span>{{ file.display_name }}</span>
            <Button
              :aria-label="`Убрать ${file.display_name}`"
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
        <label :for="`${id}-resources`">Ссылки на ресурсы</label
        ><Textarea
          :id="`${id}-resources`"
          v-model="settings.resourceUrls"
          @update:model-value="emit('resourcesChange')"
          :disabled="resourcesLocked"
          rows="2"
          placeholder="Каждая прямая ссылка — с новой строки"
        />
      </div>
      <div class="field-stack">
        <label :for="`${id}-title`">Название словаря</label
        ><InputText :id="`${id}-title`" v-model="settings.title" placeholder="Из исходного файла" />
      </div>
      <div class="field-stack">
        <label :for="`${id}-description`">Описание</label
        ><Textarea :id="`${id}-description`" v-model="settings.description" rows="2" />
      </div>
      <p v-if="error" role="alert" class="error-message">{{ error }}</p>
    </div>
    <template #footer><Button label="Готово" @click="emit('update:visible', false)" /></template>
  </Dialog>
</template>
