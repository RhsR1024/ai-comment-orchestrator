<script setup lang="ts">
import { computed } from 'vue';
import { Pause, Play, X } from 'lucide-vue-next';

import { commenterStore } from '../../lib/commenterStore';
import { run_progress_percent, run_status_label } from '../../lib/commenterView';
import { use_messages } from '../../locales/messages';

const { t } = use_messages();

const detail = computed(() => commenterStore.state.selected_run_detail);
const run = computed(() => detail.value?.run ?? null);
const progress = computed(() => (run.value ? run_progress_percent(run.value) : 0));
const can_pause = computed(() => run.value?.status === 'running');
const can_resume = computed(() => run.value?.status === 'paused');
const can_cancel = computed(() =>
  run.value ? ['running', 'paused', 'pausing'].includes(run.value.status) : false
);
async function onPause() {
  if (run.value) {
    await commenterStore.pauseRun(run.value.run_key);
  }
}

async function onResume() {
  if (run.value) {
    await commenterStore.resumeRun(run.value.run_key);
  }
}

async function onCancel() {
  if (run.value) {
    await commenterStore.cancelRun(run.value.run_key);
  }
}
</script>

<template>
  <header
    v-if="run"
    class="runbar"
    :data-status="run.status"
  >
    <div class="runbar-identity">
      <span class="runbar-project mono">{{ run.profile_key }}</span>
      <span class="status-badge runbar-status">{{ run_status_label(run.status) }}</span>
      <strong class="runbar-key mono">{{ run.run_key }}</strong>
      <span class="runbar-mode">{{ run.run_mode }}</span>
    </div>

    <div class="runbar-progress">
      <strong>{{ run.completed_jobs }} / {{ run.total_jobs }}</strong>
      <span>{{ progress }}%</span>
      <div class="runbar-progress-track"><span :style="{ width: `${progress}%` }" /></div>
    </div>

    <div class="runbar-issues">
      <span class="runbar-chip runbar-chip--review">{{ run.review_needed_jobs }} {{ t('commenter.review') }}</span>
      <span class="runbar-chip runbar-chip--failed">{{ run.failed_jobs }} {{ t('status.failed') }}</span>
      <span class="runbar-chip runbar-chip--done">{{ run.completed_jobs }} {{ t('status.completed') }}</span>
      <span class="runbar-chip runbar-chip--skipped">{{ run.skipped_jobs }} {{ t('status.skipped') }}</span>
    </div>

    <div class="runbar-actions">
      <button
        v-if="can_pause"
        type="button"
        :aria-label="t('commenter.header.pause')"
        @click="onPause"
      >
        <Pause :size="14" />
        <span>{{ t('commenter.header.pause') }}</span>
      </button>
      <button
        v-if="can_resume"
        type="button"
        :aria-label="t('commenter.header.resume')"
        @click="onResume"
      >
        <Play :size="14" />
        <span>{{ t('commenter.header.resume') }}</span>
      </button>
      <button
        v-if="can_cancel"
        type="button"
        class="runbar-cancel"
        :aria-label="t('commenter.header.cancel')"
        @click="onCancel"
      >
        <X :size="14" />
        <span>{{ t('commenter.header.cancel') }}</span>
      </button>
    </div>
  </header>

  <header
    v-else
    class="runbar runbar--idle"
  >
    <span>{{ t('commenter.header.idle') }}</span>
  </header>
</template>

<style scoped>
.runbar--idle {
  grid-template-columns: 1fr;
  color: var(--aco-muted);
}
</style>
