<script setup lang="ts">
import { onMounted, ref, useId } from 'vue';
import type {
  FormatDescriptor,
  ImportedSourceInfo,
  JobSnapshot,
  NativeBridge,
  OutputChoice,
  SourceInput,
} from '../../shared/contracts';
import { errorMessage, formatBytes } from '../../shared/format';
const props = defineProps<{ bridge: NativeBridge }>();
const emit = defineEmits<{ created: [snapshot: JobSnapshot] }>();
const id = useId();
const mode = ref('files');
const sources = ref<ImportedSourceInfo[]>([]);
const resources = ref<ImportedSourceInfo[]>([]);
const url = ref('');
const sha = ref('');
const resourceUrls = ref('');
const encoding = ref('auto');
const title = ref('');
const description = ref('');
const output = ref<OutputChoice>('app_library');
const formats = ref<FormatDescriptor[]>([]);
const busy = ref(false);
const error = ref('');
onMounted(async () => {
  try {
    formats.value = await props.bridge.listFormats();
  } catch (e) {
    error.value = errorMessage(e);
  }
});
async function pick(purpose: 'dictionaries' | 'resources' | 'resource_directory') {
  error.value = '';
  try {
    const selected = await props.bridge.pickSources(purpose);
    if (purpose === 'dictionaries') sources.value = selected;
    else
      resources.value.push(
        ...selected.filter((f) => !resources.value.some((r) => r.import_id === f.import_id)),
      );
  } catch (e) {
    error.value = errorMessage(e);
  }
}
async function pickOutput() {
  try {
    const chosen = await props.bridge.pickOutput();
    if (chosen) output.value = chosen;
  } catch (e) {
    error.value = errorMessage(e);
  }
}
function validateUrl(value: string): string {
  let parsed: URL;
  try {
    parsed = new URL(value);
  } catch {
    throw new Error('Укажите прямую ссылку HTTP или HTTPS');
  }
  if (!['http:', 'https:'].includes(parsed.protocol) || parsed.username || parsed.password)
    throw new Error('Укажите прямую ссылку HTTP или HTTPS без логина и пароля');
  return value.trim();
}
async function submit() {
  if (busy.value) return;
  error.value = '';
  busy.value = true;
  try {
    let main: SourceInput[];
    if (mode.value === 'url') {
      const validUrl = validateUrl(url.value.trim());
      if (sha.value.trim() && !/^[a-fA-F0-9]{64}$/.test(sha.value.trim()))
        throw new Error('SHA-256 должен содержать 64 шестнадцатеричных символа');
      main = [
        { kind: 'remote', url: validUrl, expected_sha256: sha.value.trim().toLowerCase() || null },
      ];
    } else {
      if (!sources.value.length) throw new Error('Выберите хотя бы один словарь');
      main = sources.value.map((f) => ({ kind: 'local', import_id: f.import_id }));
    }
    const attachments: SourceInput[] = resources.value.map((f) => ({
      kind: 'local',
      import_id: f.import_id,
    }));
    for (const line of resourceUrls.value
      .split('\n')
      .map((s) => s.trim())
      .filter(Boolean))
      attachments.push({ kind: 'remote', url: validateUrl(line), expected_sha256: null });
    // Удаляем только уже добавленные файлы: повтор отправки не создаёт дубликаты при частичной ошибке.
    for (const source of main) {
      const snapshot = await props.bridge.createJob({
        source,
        resources: attachments,
        output: output.value,
        options: { encoding: encoding.value === 'auto' ? 'auto' : { manual: encoding.value } },
        metadata_override:
          title.value.trim() || description.value.trim()
            ? {
                title: title.value.trim(),
                description: description.value.trim(),
                index_language: '',
                contents_language: '',
                source_encoding: '',
              }
            : null,
        auto_start: true,
      });
      if (source.kind === 'local')
        sources.value = sources.value.filter((f) => f.import_id !== source.import_id);
      emit('created', snapshot);
    }
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <form class="source-form" novalidate @submit.prevent="submit">
    <header class="section-intro">
      <h2>Новая конвертация</h2>
      <p class="muted">
        Добавьте словарь и ресурсы. Converter подготовит MDX и MDD на вашем устройстве.
      </p>
    </header>
    <fieldset :disabled="busy" class="source-fields">
      <legend>Источник словаря</legend>
      <div class="segmented">
        <label
          ><input v-model="mode" type="radio" value="files" :name="`${id}-source`" /> Файлы на
          устройстве</label
        ><label
          ><input v-model="mode" type="radio" value="url" :name="`${id}-source`" /> По ссылке</label
        >
      </div>
      <div v-if="mode === 'files'" class="field-stack">
        <button type="button" @click="pick('dictionaries')">Выбрать словари</button>
        <p class="hint">
          {{ formats.map((f) => f.name).join(', ') || 'DSL' }} · .dsl, .dsl.dz или ZIP. Можно
          выбрать несколько.
        </p>
        <ul v-if="sources.length" class="file-list">
          <li v-for="file in sources" :key="file.import_id">
            <span>{{ file.display_name }}</span
            ><span v-if="file.bytes !== null" class="muted">{{ formatBytes(file.bytes) }}</span>
          </li>
        </ul>
      </div>
      <div v-else class="field-stack">
        <label :for="`${id}-url`">Ссылка на словарь</label
        ><input
          :id="`${id}-url`"
          v-model="url"
          type="url"
          spellcheck="false"
          autocomplete="off"
          placeholder="https://example.org/dictionary.zip"
        />
        <p class="hint">Прямая ссылка на файл. При обрыве загрузку можно продолжить.</p>
        <label :for="`${id}-sha`">SHA-256 (необязательно)</label
        ><input
          :id="`${id}-sha`"
          v-model="sha"
          class="hash-input"
          spellcheck="false"
          autocomplete="off"
          maxlength="64"
          placeholder="Контрольная сумма от автора словаря"
        />
      </div>
    </fieldset>
    <fieldset :disabled="busy" class="source-fields">
      <legend>Изображения и озвучка</legend>
      <p class="hint">
        Уже вложенные в ZIP ресурсы будут найдены автоматически. Дополнительные файлы можно добавить
        здесь.
      </p>
      <div class="button-row">
        <button type="button" @click="pick('resources')">Выбрать файлы ресурсов</button
        ><button type="button" @click="pick('resource_directory')">Выбрать папку ресурсов</button>
      </div>
      <ul v-if="resources.length" class="file-list">
        <li v-for="file in resources" :key="file.import_id">
          <span>{{ file.display_name }}</span
          ><button
            type="button"
            class="text-button"
            :aria-label="`Убрать ${file.display_name}`"
            @click="resources = resources.filter((f) => f.import_id !== file.import_id)"
          >
            Убрать
          </button>
        </li>
      </ul>
      <label :for="`${id}-resources`">Ссылки на ресурсы</label
      ><textarea
        :id="`${id}-resources`"
        v-model="resourceUrls"
        rows="2"
        spellcheck="false"
        placeholder="По одной ссылке на ZIP или файл в строке"
      />
    </fieldset>
    <fieldset :disabled="busy" class="source-fields">
      <legend>Параметры и результат</legend>
      <div class="field-grid">
        <div>
          <label :for="`${id}-encoding`">Кодировка DSL</label
          ><select :id="`${id}-encoding`" v-model="encoding">
            <option value="auto">Определить автоматически</option>
            <option value="utf-8">UTF-8</option>
            <option value="utf-16le">UTF-16 LE</option>
            <option value="utf-16be">UTF-16 BE</option>
            <option value="windows-1251">Windows-1251</option>
          </select>
        </div>
        <div>
          <label :for="`${id}-title`">Название (необязательно)</label
          ><input :id="`${id}-title`" v-model="title" placeholder="Из заголовка словаря" />
        </div>
      </div>
      <label :for="`${id}-description`">Описание (необязательно)</label
      ><textarea :id="`${id}-description`" v-model="description" rows="2" />
      <div class="destination">
        <div>
          <strong>Куда сохранить</strong>
          <p class="hint destination-path">
            {{ output === 'app_library' ? 'Библиотека приложения' : output.desktop_folder }}
          </p>
        </div>
        <button type="button" @click="pickOutput">Выбрать папку</button>
      </div>
      <p class="hint">Готовые файлы можно сохранить в другую папку после конвертации.</p>
    </fieldset>
    <p v-if="error" role="alert" class="error-message">{{ error }}</p>
    <footer class="form-footer">
      <span class="muted">MDX · MDD · отчёт JSON</span
      ><button type="submit" class="primary" :disabled="busy">
        {{ busy ? 'Добавление…' : 'Добавить и начать' }}
      </button>
    </footer>
  </form>
</template>
