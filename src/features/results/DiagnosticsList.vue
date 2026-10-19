<script setup lang="ts">
import { errorState } from '../../shared/i18n/errors';

import { useI18n } from 'vue-i18n';
const { t } = useI18n();

import { computed, onBeforeUnmount, ref, watch } from 'vue';
import Message from 'primevue/message';
import Paginator, { type PageState } from 'primevue/paginator';
import type { DiagnosticPage, NativeBridge } from '../../shared/contracts';
import { errorMessage, formatCount } from '../../shared/format';
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
const error = errorState();
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
    if (run === generation) error.value = e;
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
      {{ t('app.log') }} · {{ formatCount(count) }}
      <span>{{ open ? t('app.hide') : t('app.show') }}</span>
    </button>
    <div v-if="isOpen || preview" class="diagnostics-body" :aria-busy="busy">
      <div v-if="error">
        <Message severity="error">{{ error }}</Message>
        <button type="button" class="diagnostics-retry" @click="load">
          {{ t('app.retry_read') }}
        </button>
      </div>
      <p v-else-if="busy" role="status" class="muted">{{ t('app.reading_log') }}</p>
      <p v-else-if="!page.items.length" class="muted">
        {{ count ? t('app.log_missing') : t('app.no_warnings') }}
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
              <span>{{ errorMessage(item) }}</span
              >{{ ' '
              }}<a
                v-if="preview && count > 1"
                :href="`#diagnostics-${id}`"
                :aria-controls="`diagnostics-${id}`"
                aria-haspopup="dialog"
                class="diagnostics-more"
                @click.prevent="showAll"
              >
                {{ t('app.more_warnings', { count: formatCount(count - 1) }, count - 1) }}</a
              >
            </p>
            <p v-if="(item.occurrences ?? 1) > 1" class="hint">
              {{
                t(
                  'app.occurrences',
                  { count: formatCount(item.occurrences ?? 1) },
                  item.occurrences ?? 1,
                )
              }}
            </p>
            <span v-if="item.source_location" class="hint"
              >{{ (item.occurrences ?? 1) > 1 ? t('app.first_mention') : ''
              }}{{
                t('app.source_line', {
                  source: item.source_location.source,
                  line: item.source_location.line,
                })
              }}</span
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
        :current-page-report-template="
          t('app.page_report', { first: '{first}', last: '{last}', totalRecords: '{totalRecords}' })
        "
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
        :aria-label="t('app.log_pages')"
        class="pagination"
      >
        <button type="button" :disabled="busy || !previous.length" @click="back">
          {{ t('app.back') }}</button
        ><span class="muted"
          >{{ formatCount(offset + 1) }}–{{ formatCount(offset + page.items.length) }}
          {{ t('app.of') }} {{ formatCount(page.total) }}</span
        ><button type="button" :disabled="busy || page.next_offset === null" @click="next">
          {{ t('app.next') }}
        </button>
      </nav>
    </div>
  </section>
</template>
