<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import Message from 'primevue/message';
import Paginator, { type PageState } from 'primevue/paginator';
import type { DiagnosticPage, NativeBridge } from '../../shared/contracts';
import { errorMessage, formatCount, formatQuantity } from '../../shared/format';
const props = defineProps<{
  id: string;
  count: number;
  bridge: NativeBridge;
  severity?: 'warn' | 'error';
  preview?: boolean;
  expanded?: boolean;
}>();
const emit = defineEmits<{ showAll: [] }>();
const open = ref(false);
const isOpen = computed(() => props.expanded || open.value);
const offset = ref(0);
const previous = ref<number[]>([]);
const page = ref<DiagnosticPage>({ items: [], next_offset: null, total: 0 });
const busy = ref(false);
const error = ref('');
let generation = 0;
async function load() {
  const run = ++generation;
  busy.value = true;
  error.value = '';
  try {
    const result = await props.bridge.readDiagnostics(
      props.id,
      offset.value,
      isOpen.value ? 50 : 1,
    );
    if (run === generation) page.value = result;
  } catch (e) {
    if (run === generation) error.value = errorMessage(e);
  } finally {
    if (run === generation) busy.value = false;
  }
}
watch(
  () => props.id,
  () => {
    generation++;
    page.value = { items: [], next_offset: null, total: 0 };
    offset.value = 0;
    previous.value = [];
    if (isOpen.value || props.preview) void load();
  },
);
watch(
  [isOpen, () => props.count],
  () => {
    offset.value = 0;
    previous.value = [];
    if (isOpen.value || props.preview) void load();
  },
  { immediate: true },
);
onBeforeUnmount(() => {
  generation++;
});
function next() {
  if (page.value.next_offset === null) return;
  previous.value.push(offset.value);
  offset.value = page.value.next_offset;
  void load();
}
function back() {
  offset.value = previous.value.pop() ?? 0;
  void load();
}
function changePage(event: PageState) {
  offset.value = event.first;
  void load();
}
function showAll(event: MouseEvent) {
  // WebKit не фокусирует ссылку при клике; Dialog сохраняет текущий фокус для возврата.
  (event.currentTarget as HTMLAnchorElement).focus();
  emit('showAll');
}
</script>
<template>
  <section class="diagnostics">
    <button
      v-if="!preview && !expanded"
      type="button"
      class="disclosure"
      :aria-expanded="open"
      @click="open = !open"
    >
      Журнал · {{ formatCount(count) }}
      <span>{{ open ? 'Скрыть' : 'Показать' }}</span>
    </button>
    <div v-if="isOpen || preview" class="diagnostics-body" :aria-busy="busy">
      <div v-if="error">
        <Message severity="error">{{ error }}</Message>
        <button type="button" class="diagnostics-retry" @click="load">Повторить чтение</button>
      </div>
      <p v-else-if="busy" role="status" class="muted">Чтение журнала…</p>
      <p v-else-if="!page.items.length" class="muted">
        {{ count ? 'Не удалось найти записи журнала.' : 'Предупреждений нет.' }}
      </p>
      <ol
        v-else
        class="diagnostic-list"
        :class="{ 'diagnostic-list-preview': preview }"
        :start="offset + 1"
      >
        <li
          v-for="(item, index) in preview ? page.items.slice(0, 1) : page.items"
          :key="`${offset}-${index}`"
        >
          <component
            :is="preview ? Message : 'div'"
            :severity="preview ? (severity ?? 'warn') : undefined"
          >
            <p>
              <span>{{ item.message }}</span
              >{{ ' '
              }}<a
                v-if="preview && count > 1"
                :href="`#diagnostics-${id}`"
                :aria-controls="`diagnostics-${id}`"
                aria-haspopup="dialog"
                class="diagnostics-more"
                @click.prevent="showAll"
              >
                (ещё
                {{
                  formatQuantity(count - 1, ['предупреждение', 'предупреждения', 'предупреждений'])
                }})</a
              >
            </p>
            <p v-if="(item.occurrences ?? 1) > 1" class="hint">
              Встречается {{ formatQuantity(item.occurrences ?? 1, ['раз', 'раза', 'раз']) }}.
            </p>
            <span v-if="item.source_location" class="hint"
              >{{ (item.occurrences ?? 1) > 1 ? 'Первое упоминание: ' : ''
              }}{{ item.source_location.source }}, строка {{ item.source_location.line }}</span
            >
          </component>
        </li>
      </ol>
      <Paginator
        v-if="expanded && !busy && !error && page.total > 50"
        :first="offset"
        :rows="50"
        :total-records="page.total"
        template="FirstPageLink PrevPageLink CurrentPageReport NextPageLink LastPageLink"
        current-page-report-template="{first}–{last} из {totalRecords}"
        @page="changePage"
      />
      <nav
        v-else-if="
          !expanded &&
          !preview &&
          isOpen &&
          !busy &&
          !error &&
          (previous.length || page.next_offset !== null)
        "
        aria-label="Страницы журнала"
        class="pagination"
      >
        <button type="button" :disabled="busy || !previous.length" @click="back">Назад</button
        ><span class="muted"
          >{{ formatCount(offset + 1) }}–{{ formatCount(offset + page.items.length) }} из
          {{ formatCount(page.total) }}</span
        ><button type="button" :disabled="busy || page.next_offset === null" @click="next">
          Далее
        </button>
      </nav>
    </div>
  </section>
</template>
