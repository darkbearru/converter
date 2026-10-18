<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue';
import type { DiagnosticPage, NativeBridge } from '../../shared/contracts';
import { errorMessage, formatCount } from '../../shared/format';
const props = defineProps<{ id: string; count: number; bridge: NativeBridge }>();
const open = ref(false);
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
    const result = await props.bridge.readDiagnostics(props.id, offset.value, 50);
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
    if (open.value) void load();
  },
);
watch([open, () => props.count], () => {
  if (open.value) void load();
});
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
</script>
<template>
  <section class="diagnostics">
    <button type="button" class="disclosure" :aria-expanded="open" @click="open = !open">
      Журнал · {{ formatCount(count) }} <span>{{ open ? 'Скрыть' : 'Показать' }}</span>
    </button>
    <div v-if="open" :aria-busy="busy">
      <p v-if="error" role="alert" class="error-message">{{ error }}</p>
      <p v-if="busy" role="status" class="muted">Чтение журнала…</p>
      <p v-else-if="!page.items.length" class="muted">Предупреждений нет.</p>
      <ol v-else class="diagnostic-list" :start="offset + 1">
        <li v-for="(item, index) in page.items" :key="`${offset}-${index}`">
          <p>{{ item.message }}</p>
          <span v-if="item.source_location" class="hint"
            >{{ item.source_location.source }}, строка {{ item.source_location.line }}</span
          >
        </li>
      </ol>
      <nav
        v-if="previous.length || page.next_offset !== null"
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
