<script setup lang="ts">
import { computed } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { Box, ClipboardCheck, Globe2, SquareActivity } from 'lucide-vue-next';

import { commenterStore } from '../lib/commenterStore';
import { use_messages } from '../locales/messages';

const route = useRoute();
const router = useRouter();
const { t } = use_messages();

const profile_count = computed(() => commenterStore.state.profiles.length);
const review_count = computed(() => commenterStore.state.review_jobs.length);
const run_attention_count = computed(
  () =>
    commenterStore.state.runs.filter((run) => !run.finished_at).length
);

const has_token = computed(() =>
  Boolean(commenterStore.state.app_settings?.api_bearer_token.trim())
);
const api_status_label = computed(() =>
  has_token.value ? t('sidebar.apiOnline') : t('sidebar.apiMissing')
);
const concurrency_used = computed(
  () => commenterStore.state.runs.filter((run) => !run.finished_at).length
);
const concurrency_max = computed(
  () => commenterStore.state.app_settings?.api_concurrency_limit ?? 0
);
const capacity_label = computed(() =>
  t('sidebar.capacity', { used: concurrency_used.value, max: concurrency_max.value })
);

function is_active(path: string) {
  return route.path === path;
}

function go(path: string) {
  void router.push(path);
}
</script>

<template>
  <aside class="sidebar">
    <div class="sidebar-brand">
      <div class="sidebar-mark">AC</div>
      <div>
        <strong>ACO</strong>
        <p>Comment Orchestrator</p>
      </div>
    </div>

    <nav class="sidebar-nav" aria-label="Workspaces">
      <button
        class="sidebar-link"
        :class="{ active: is_active('/settings') }"
        @click="go('/settings')"
      >
        <Box :size="15" />
        <span>{{ t('nav.projectConfig') }}</span>
        <span class="sidebar-count">{{ profile_count }}</span>
      </button>
      <button
        class="sidebar-link"
        :class="{ active: is_active('/workspace') }"
        @click="go('/workspace')"
      >
        <SquareActivity :size="15" />
        <span>{{ t('nav.runWorkspace') }}</span>
        <span class="sidebar-count sidebar-count--active">{{ run_attention_count }}</span>
      </button>
      <button
        class="sidebar-link"
        :class="{ active: is_active('/review') }"
        @click="go('/review')"
      >
        <ClipboardCheck :size="15" />
        <span>{{ t('commenter.review') }}</span>
        <span class="sidebar-count sidebar-count--active">{{ review_count }}</span>
      </button>
      <button
        class="sidebar-link"
        :class="{ active: is_active('/global') }"
        @click="go('/global')"
      >
        <Globe2 :size="15" />
        <span>{{ t('nav.globalSettings') }}</span>
      </button>
    </nav>

    <div class="sidebar-spacer" />

    <div class="sidebar-status-card">
      <div class="sidebar-status-line">
        <span
          class="sidebar-status-dot"
          :class="has_token ? 'sidebar-status-dot--online' : 'sidebar-status-dot--offline'"
        />
        <strong>{{ api_status_label }}</strong>
      </div>
      <p>{{ capacity_label }}</p>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  display: flex;
  flex-direction: column;
  gap: 18px;
  min-height: 100vh;
  background: var(--aco-surface-1);
  border-right: 1px solid var(--aco-border);
  padding: 18px 14px;
}

.sidebar-brand {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 8px 14px;
}

.sidebar-brand strong {
  display: block;
  color: var(--aco-text);
  font-size: 13px;
  font-weight: 700;
  line-height: 1.25;
}

.sidebar-brand p {
  margin: 2px 0 0;
  color: var(--aco-muted);
  font-size: 11px;
}

.sidebar-mark {
  display: grid;
  place-items: center;
  width: 32px;
  height: 32px;
  border-radius: 8px;
  background: var(--aco-surface-3);
  color: var(--aco-teal);
  font-size: 12px;
  font-weight: 700;
}

.sidebar-nav {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.sidebar-link {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--aco-muted);
  cursor: pointer;
  padding: 8px 10px;
  text-align: left;
  font-size: 13px;
}

.sidebar-link:hover {
  background: var(--aco-surface-2);
  color: var(--aco-text);
}

.sidebar-link.active {
  background: var(--aco-surface-2);
  color: var(--aco-text);
}

.sidebar-count {
  margin-left: auto;
  border-radius: 999px;
  background: var(--aco-surface-3);
  color: var(--aco-muted);
  padding: 1px 7px;
  font-size: 11px;
}

.sidebar-count--active {
  color: var(--aco-yellow);
}

.sidebar-spacer {
  flex: 1;
}

.sidebar-status-card {
  display: flex;
  flex-direction: column;
  gap: 4px;
  border: 1px solid var(--aco-border);
  border-radius: 8px;
  background: var(--aco-surface-2);
  padding: 8px 10px;
  font-size: 12px;
  color: var(--aco-muted);
}

.sidebar-status-line {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--aco-text);
}

.sidebar-status-dot {
  width: 6px;
  height: 6px;
  border-radius: 999px;
}

.sidebar-status-dot--online {
  background: var(--aco-green);
}

.sidebar-status-dot--offline {
  background: var(--aco-red);
}
</style>
