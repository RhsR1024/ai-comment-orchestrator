<script setup lang="ts">
import { use_messages } from '../locales/messages';

defineProps<{
  titleKey: string;
  subtitleKey: string;
}>();

const { t } = use_messages();
</script>

<template>
  <section class="page-shell">
    <div class="page-intro">
      <h2>{{ t(titleKey) }}</h2>
      <p>{{ t(subtitleKey) }}</p>
    </div>
    <div class="panel">
      <div class="empty-state">
        {{ t('commenter.empty') }}
      </div>
    </div>
  </section>
</template>
