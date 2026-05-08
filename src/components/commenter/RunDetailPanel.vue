<script setup lang="ts">
import { computed, ref, watch } from 'vue';

import { commenterStore } from '../../lib/commenterStore';
import { streamSliceKey } from '../../lib/commenterStreamSlice';
import StreamContentPanel from './StreamContentPanel.vue';
import WorkspaceTreePanel from './WorkspaceTreePanel.vue';
import QueueRunsTable from './QueueRunsTable.vue';
import { use_messages } from '../../locales/messages';

const props = withDefaults(
  defineProps<{ variant?: 'stacked' | 'reference' }>(),
  {
    variant: 'stacked'
  }
);

const { t } = use_messages();
const active_left_tab = ref<'files' | 'runs'>('files');

const detail = computed(() => commenterStore.state.selected_run_detail);
const run = computed(() => detail.value?.run ?? null);

const file_count = computed(() => detail.value?.jobs.length ?? 0);
const run_count = computed(() => commenterStore.state.runs.length);

const follow_mode = ref<'live' | 'locked'>('live');
const selected_file = ref<string | null>(null);

watch(
  () => run.value?.run_key ?? null,
  () => {
    follow_mode.value = 'live';
    selected_file.value = run.value?.current_file ?? null;
  },
  { immediate: true }
);

watch(
  () => run.value?.current_file ?? null,
  (current) => {
    if (follow_mode.value === 'live') {
      selected_file.value = current;
    }
  },
  { immediate: true }
);

function onTreeSelect(relative_path: string) {
  follow_mode.value = 'locked';
  selected_file.value = relative_path;
}

const live_slice = computed(() => {
  if (!run.value || !selected_file.value) {
    return null;
  }
  return commenterStore.state.live_streams.get(streamSliceKey(run.value.run_key, selected_file.value)) ?? null;
});

const selected_job = computed(() => {
  if (!detail.value || !selected_file.value) {
    return null;
  }
  return detail.value.jobs.find((job) => job.relative_path === selected_file.value) ?? null;
});

const job_status = computed(() => {
  if (live_slice.value?.status === 'streaming') {
    return 'streaming' as const;
  }
  if (live_slice.value?.status === 'failed') {
    return 'failed' as const;
  }
  return selected_job.value?.status ?? 'idle';
});

const error_message = computed(
  () => selected_job.value?.error_message ?? live_slice.value?.error ?? null
);
</script>

<template>
  <div v-if="props.variant === 'reference'" class="run-detail-reference-grid">
    <aside class="run-left-rail">
      <nav class="run-left-tabs" aria-label="左侧视图切换">
        <button
          type="button"
          :class="{ active: active_left_tab === 'files' }"
          @click="active_left_tab = 'files'"
        >
          {{ t('commenter.files') }}
          <span class="left-tab-count">{{ file_count }}</span>
        </button>
        <button
          type="button"
          :class="{ active: active_left_tab === 'runs' }"
          @click="active_left_tab = 'runs'"
        >
          {{ t('commenter.runs') }}
          <span class="left-tab-count">{{ run_count }}</span>
        </button>
      </nav>
      <div class="run-left-body">
        <WorkspaceTreePanel
          v-if="active_left_tab === 'files'"
          @select-file="onTreeSelect"
        />
        <QueueRunsTable
          v-if="active_left_tab === 'runs'"
          variant="rail"
        />
      </div>
    </aside>
    <main class="run-stream-rail">
      <StreamContentPanel
        :mode="follow_mode"
        :run_key="run?.run_key ?? null"
        :relative_path="selected_file"
        :live_text="live_slice?.text ?? ''"
        :stream-last-chunk-at="live_slice?.last_chunk_at ?? null"
        :status="job_status"
        :error_message="error_message"
      />
    </main>
  </div>

  <div v-else class="run-detail-grid">
    <WorkspaceTreePanel @select-file="onTreeSelect" />
    <StreamContentPanel
      :mode="follow_mode"
      :run_key="run?.run_key ?? null"
      :relative_path="selected_file"
      :live_text="live_slice?.text ?? ''"
      :stream-last-chunk-at="live_slice?.last_chunk_at ?? null"
      :status="job_status"
      :error_message="error_message"
    />
  </div>
</template>

<style scoped>
.run-detail-grid {
  display: grid;
  grid-template-columns: 280px 1fr;
  gap: 12px;
  min-height: 420px;
}

@media (max-width: 960px) {
  .run-detail-grid {
    grid-template-columns: 1fr;
  }
}
</style>
