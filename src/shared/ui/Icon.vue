<script setup lang="ts">
import { computed } from 'vue';
import close from '../../assets/icons/007__close.svg';
import refresh from '../../assets/icons/019__refresh.svg';
import download from '../../assets/icons/027__download.svg';
import upload from '../../assets/icons/030__cloud_upload.svg';
import remove from '../../assets/icons/086__empty.svg';
import chevron from '../../assets/icons/004__arrow_down.svg';
import caution from '../../assets/icons/042__caution.svg';
import link from '../../assets/icons/069__hyperlink.svg';
import settings from '../../assets/icons/082__setting_cog.svg';
import play from '../../assets/icons/131__play.svg';
import pause from '../../assets/icons/132__pause.svg';
import stop from '../../assets/icons/133__stop.svg';
import folder from '../../assets/icons/139__open file.svg';
import paste from '../../assets/icons/paste_icon.svg';
import cash from '../../assets/icons/126__cash.svg';
import mail from '../../assets/icons/049__mail.svg';
const icons = {
  chevron,
  close,
  refresh,
  download,
  upload,
  remove,
  caution,
  link,
  settings,
  play,
  pause,
  stop,
  folder,
  paste,
  cash,
  mail,
};
const props = defineProps<{ name: keyof typeof icons; label?: string }>();
const mask = computed(() => `url("${icons[props.name]}")`);
</script>
<template>
  <span
    class="app-icon"
    :data-icon="name"
    :role="label ? 'img' : undefined"
    :aria-label="label"
    :aria-hidden="label ? undefined : true"
    :style="{ '--icon-image': mask }"
  />
</template>
