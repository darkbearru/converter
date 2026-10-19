<script setup lang="ts">
import { errorState } from '../../shared/i18n/errors';

import { useI18n } from 'vue-i18n';
const { t } = useI18n();

import { computed, onBeforeUnmount, onMounted, ref, useId } from 'vue';
import Dialog from 'primevue/dialog';
import Button from 'primevue/button';
import Message from 'primevue/message';
import Select from 'primevue/select';
import type { NativeBridge } from '../../shared/contracts';
import { dialogCloseButtonProps } from '../../shared/ui/dialog';
import Icon from '../../shared/ui/Icon.vue';
import { useLocalization, type LanguagePreference } from '../../shared/i18n';
import bookIcon from '../../../book.svg';

const props = defineProps<{ bridge: NativeBridge; busy?: boolean }>();
const localization = useLocalization();
const languageId = useId();
const changingLanguage = ref(false);
const languageChoice = ref<LanguagePreference>(localization.preferences.language);
const languageOptions = computed(() => [
  { label: t('app.system_language'), value: 'system' },
  { label: 'Русский', value: 'ru' },
  { label: 'English', value: 'en' },
]);
async function changeLanguage(language: LanguagePreference) {
  changingLanguage.value = true;
  actionError.value = '';
  try {
    await localization.setLanguage(language);
  } catch (error) {
    actionError.value = error;
  } finally {
    changingLanguage.value = false;
    languageChoice.value = localization.preferences.language;
  }
}
const about = ref(false);
const notification = ref(false);
const version = ref('');
const available = ref<string | null>(null);
const status = ref<'checking' | 'current' | 'available' | 'error'>('checking');
const actionError = errorState();
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
  if (status.value === 'checking') return t('app.checking_updates');
  if (status.value === 'error') return t('app.update_check_failed');
  if (available.value) return t('app.update_available_inline', { version: available.value });
  return t('app.up_to_date');
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
    actionError.value = error;
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
    :header="t('app.about')"
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
            {{ t('app.version', { version: version || '…' }) }}
            <span class="about-updates" aria-live="polite">({{ updateDescription }})</span>
          </p>
        </div>
      </div>
      <p class="about-description">{{ t('app.about_description') }}</p>
      <div class="about-attribution">
        <p class="about-copyright">© Alexey Abramenko</p>
        <a
          class="about-thanks"
          v-if="localization.preferences.support_url"
          :href="localization.preferences.support_url"
          @click.prevent="openPage('support')"
          ><Icon name="cash" /><span> {{ t('app.thanks') }} </span></a
        >
      </div>
      <div class="about-language">
        <label :id="`${languageId}-label`" :for="languageId">{{ t('app.language') }}</label>
        <Select
          :input-id="languageId"
          :aria-labelledby="`${languageId}-label`"
          v-model="languageChoice"
          :options="languageOptions"
          option-label="label"
          option-value="value"
          :disabled="changingLanguage"
          @update:model-value="changeLanguage"
        >
          <template #dropdownicon><Icon name="chevron" /></template>
        </Select>
      </div>
      <Message v-if="actionError" severity="error">{{ actionError }}</Message>
    </div>
    <template #footer>
      <Button
        v-if="!available"
        :label="t('app.check_updates')"
        severity="secondary"
        outlined
        :disabled="status === 'checking'"
        @click="check()"
      />
      <Button
        v-if="available"
        :label="t('app.download')"
        severity="secondary"
        outlined
        :disabled="opening"
        @click="openPage('release')"
      />
      <Button :label="t('app.close')" severity="secondary" outlined @click="about = false" />
    </template>
  </Dialog>
  <Dialog
    v-model:visible="notificationVisible"
    modal
    :header="t('app.update_available')"
    class="app-dialog update-dialog"
    :close-button-props="dialogCloseButtonProps"
  >
    <template #closeicon><Icon name="close" /></template>
    <div class="update-content" tabindex="-1" autofocus>
      <p>{{ t('app.new_version', { version: available ?? '' }) }}</p>
      <p class="update-description">
        {{ t('app.installed_version', { version }) }}
      </p>
      <Message v-if="actionError" severity="error">{{ actionError }}</Message>
    </div>
    <template #footer>
      <Button :label="t('app.later')" severity="secondary" outlined @click="notification = false" />
      <Button
        :label="t('app.download')"
        severity="secondary"
        outlined
        :disabled="opening"
        @click="openPage('release')"
      />
    </template>
  </Dialog>
</template>
