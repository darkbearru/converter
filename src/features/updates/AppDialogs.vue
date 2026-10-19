<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import Dialog from 'primevue/dialog';
import Button from 'primevue/button';
import Message from 'primevue/message';
import type { NativeBridge } from '../../shared/contracts';
import { dialogCloseButtonProps } from '../../shared/ui/dialog';
import Icon from '../../shared/ui/Icon.vue';
import { errorMessage } from '../../shared/format';
import bookIcon from '../../../book.svg';

const props = defineProps<{ bridge: NativeBridge; busy?: boolean }>();
const about = ref(false);
const notification = ref(false);
const version = ref('');
const available = ref<string | null>(null);
const status = ref<'checking' | 'current' | 'available' | 'error'>('checking');
const actionError = ref('');
const opening = ref(false);
let disposed = false;
let unlisten: (() => void) | undefined;
let pending: Promise<void> | undefined;
const notificationVisible = computed({
  get: () => notification.value && !about.value && !props.busy,
  set: (visible) => {
    notification.value = visible;
  },
});
const updateDescription = computed(() => {
  if (status.value === 'checking') return 'проверка обновлений…';
  if (status.value === 'error') return 'не удалось проверить обновления';
  if (available.value) return `доступна версия ${available.value}`;
  return 'установлена последняя версия';
});

function check(startup = false): Promise<void> {
  if (pending) return pending;
  status.value = 'checking';
  pending = (async () => {
    try {
      const result = await props.bridge.checkForUpdates();
      if (disposed) return;
      available.value = result.version;
      status.value = result.version ? 'available' : 'current';
      if (startup && result.version && !about.value) notification.value = true;
    } catch {
      if (!disposed) status.value = 'error';
    } finally {
      pending = undefined;
    }
  })();
  return pending;
}
async function openPage(kind: 'release' | 'support') {
  if (opening.value) return;
  opening.value = true;
  actionError.value = '';
  try {
    if (kind === 'release') await props.bridge.openReleasePage();
    else await props.bridge.openSupportPage();
    if (kind === 'release') notification.value = false;
  } catch (error) {
    actionError.value = errorMessage(error);
  } finally {
    opening.value = false;
  }
}
onMounted(async () => {
  try {
    const stop = await props.bridge.onAboutRequested(() => {
      notification.value = false;
      actionError.value = '';
      about.value = true;
    });
    if (disposed) {
      stop();
      return;
    }
    unlisten = stop;
    version.value = await props.bridge.getAppVersion();
    if (!disposed) await check(true);
  } catch {
    if (!disposed) status.value = 'error';
  }
});
onBeforeUnmount(() => {
  disposed = true;
  unlisten?.();
});
</script>

<template>
  <Dialog
    v-model:visible="about"
    modal
    header="О программе"
    class="app-dialog about-dialog"
    :close-button-props="dialogCloseButtonProps"
  >
    <template #closeicon><Icon name="close" /></template>
    <div class="about-content" tabindex="-1" autofocus>
      <div class="about-identity">
        <img :src="bookIcon" alt="" width="58" height="52" />
        <div>
          <h2>Converter</h2>
          <p>
            Версия {{ version || '…' }}
            <span class="about-updates" aria-live="polite">({{ updateDescription }})</span>
          </p>
        </div>
      </div>
      <p class="about-description">Локальный конвертер словарей DSL в MDX и MDD.</p>
      <div class="about-attribution">
        <p class="about-copyright">© Alexey Abramenko</p>
        <a
          class="about-thanks"
          href="https://pay.cloudtips.ru/p/7e782291"
          @click.prevent="openPage('support')"
          ><Icon name="cash" /><span>Сказать спасибо</span></a
        >
      </div>
      <Message v-if="actionError" severity="error">{{ actionError }}</Message>
    </div>
    <template #footer>
      <Button
        v-if="!available"
        label="Проверить обновления"
        severity="secondary"
        outlined
        :disabled="status === 'checking'"
        @click="check()"
      />
      <Button
        v-if="available"
        label="Скачать"
        severity="secondary"
        outlined
        :disabled="opening"
        @click="openPage('release')"
      />
      <Button label="Закрыть" severity="secondary" outlined @click="about = false" />
    </template>
  </Dialog>
  <Dialog
    v-model:visible="notificationVisible"
    modal
    header="Доступно обновление"
    class="app-dialog update-dialog"
    :close-button-props="dialogCloseButtonProps"
  >
    <template #closeicon><Icon name="close" /></template>
    <div class="update-content" tabindex="-1" autofocus>
      <p>Доступна новая версия Converter {{ available }}.</p>
      <p class="update-description">
        Сейчас установлена версия {{ version }}. Установщики для macOS и Windows доступны на GitHub.
      </p>
      <Message v-if="actionError" severity="error">{{ actionError }}</Message>
    </div>
    <template #footer>
      <Button label="Позже" severity="secondary" outlined @click="notification = false" />
      <Button
        label="Скачать"
        severity="secondary"
        outlined
        :disabled="opening"
        @click="openPage('release')"
      />
    </template>
  </Dialog>
</template>
