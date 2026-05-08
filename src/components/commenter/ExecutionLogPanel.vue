<script setup lang="ts">
import { computed, ref } from 'vue';
import {
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  CircleDashed,
  Loader2,
  TriangleAlert,
  XCircle,
  Zap
} from 'lucide-vue-next';

import { buildFileLogEntries, type FileLogEntry, type PhaseTag } from '../../lib/commenterFileLog';
import { commenterStore } from '../../lib/commenterStore';
import { run_status_label } from '../../lib/commenterView';
import { use_messages } from '../../locales/messages';

const props = withDefaults(
  defineProps<{ variant?: 'panel' | 'rail' }>(),
  {
    variant: 'panel'
  }
);

const emit = defineEmits<{ 'select-file': [string] }>();

const { t } = use_messages();

const detail = computed(() => commenterStore.state.selected_run_detail);
const expanded = ref<Set<string>>(new Set());

const entries = computed<FileLogEntry[]>(() => {
  if (!detail.value) {
    return [];
  }
  return buildFileLogEntries(detail.value.jobs, detail.value.events).slice(0, 200);
});

function toggle(path: string) {
  const next = new Set(expanded.value);
  if (next.has(path)) {
    next.delete(path);
  } else {
    next.add(path);
  }
  expanded.value = next;
}

function onSelect(entry: FileLogEntry) {
  emit('select-file', entry.relative_path);
}

function statusIcon(entry: FileLogEntry) {
  switch (entry.status) {
    case 'done':
      return CheckCircle2;
    case 'failed':
      return XCircle;
    case 'review_needed':
      return TriangleAlert;
    case 'requesting':
    case 'validating':
    case 'writing':
      return Zap;
    case 'pending':
    case 'leased':
    case 'retry_waiting':
      return CircleDashed;
    default:
      return Loader2;
  }
}

function elapsed(entry: FileLogEntry): string {
  if (entry.started_at === null || entry.ended_at === null) {
    return '';
  }
  const ms = entry.ended_at - entry.started_at;
  if (ms <= 0) {
    return '';
  }
  return ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} s`;
}

function phaseLabel(phase: PhaseTag): string {
  return t(`commenter.log.phase.${phase}`);
}

function phaseToggleLabel(path: string): string {
  return expanded.value.has(path) ? t('commenter.log.collapse') : t('commenter.log.expand');
}
</script>

<template>
  <section :class="['execution-log-panel', { 'execution-log-panel--rail': props.variant === 'rail' }]">
    <header v-if="props.variant !== 'rail'" class="execution-log-header">
      <div>
        <h3>{{ t('commenter.logs') }}</h3>
        <p>{{ t('commenter.logs.help') }}</p>
      </div>
      <span
        v-if="detail"
        class="status-badge"
      >
        {{ run_status_label(detail.run.status) }}
      </span>
    </header>

    <ul
      v-if="entries.length > 0"
      class="execution-log-list"
    >
      <li
        v-for="entry in entries"
        :key="entry.relative_path"
        class="execution-log-row"
        :class="`execution-log-row--${entry.status}`"
      >
        <button
          type="button"
          class="execution-log-toggle"
          :aria-expanded="expanded.has(entry.relative_path)"
          :aria-label="phaseToggleLabel(entry.relative_path)"
          @click="toggle(entry.relative_path)"
        >
          <component
            :is="expanded.has(entry.relative_path) ? ChevronDown : ChevronRight"
            :size="12"
          />
        </button>
        <button
          type="button"
          class="execution-log-main"
          @click="onSelect(entry)"
        >
          <component
            :is="statusIcon(entry)"
            :size="14"
          />
          <span class="execution-log-path">{{ entry.relative_path }}</span>
          <span class="execution-log-elapsed">{{ elapsed(entry) }}</span>
        </button>
        <ul
          v-if="expanded.has(entry.relative_path)"
          class="execution-log-phases"
        >
          <li
            v-for="phase in entry.phases"
            :key="`${phase.phase}-${phase.at}`"
            :class="`execution-log-phase execution-log-phase--${phase.level}`"
          >
            <div class="execution-log-phase-copy">
              <span>{{ phaseLabel(phase.phase) }}</span>
              <small v-if="phase.message">{{ phase.message }}</small>
            </div>
            <small>{{ new Date(phase.at).toLocaleTimeString() }}</small>
          </li>
          <li
            v-if="entry.error_message"
            class="execution-log-phase execution-log-phase--error"
          >
            <span>{{ entry.error_message }}</span>
          </li>
        </ul>
      </li>
    </ul>

    <div
      v-else
      class="empty-state"
    >
      {{ t('commenter.log.empty') }}
    </div>
  </section>
</template>

<style scoped>
.execution-log-panel {
  display: flex;
  flex-direction: column;
  min-height: 360px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  background: rgba(5, 15, 18, 0.58);
}

.execution-log-header {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  align-items: flex-start;
  padding: 12px 14px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}

.execution-log-header h3,
.execution-log-header p {
  margin: 0;
}

.execution-log-header p {
  margin-top: 4px;
  color: #88a5a1;
  font-size: 12px;
}

.execution-log-list {
  list-style: none;
  margin: 0;
  padding: 8px;
  overflow: auto;
  display: grid;
  gap: 8px;
  flex: 1;
  min-height: 0;
}

.execution-log-row {
  display: grid;
  grid-template-columns: 18px 1fr;
  gap: 6px;
  padding: 8px;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.03);
}

.execution-log-row--done {
  background: rgba(34, 197, 94, 0.08);
}

.execution-log-row--failed {
  background: rgba(239, 68, 68, 0.08);
}

.execution-log-row--review_needed {
  background: rgba(245, 158, 11, 0.08);
}

.execution-log-row--requesting,
.execution-log-row--validating,
.execution-log-row--writing {
  background: rgba(34, 197, 94, 0.06);
}

.execution-log-toggle,
.execution-log-main {
  background: none;
  border: 0;
  color: inherit;
  cursor: pointer;
  padding: 0;
  text-align: left;
}

.execution-log-main {
  display: grid;
  grid-template-columns: 16px minmax(0, 1fr) auto;
  gap: 8px;
  align-items: center;
  font-family:
    ui-monospace,
    SFMono-Regular,
    Menlo,
    Monaco,
    Consolas,
    "Liberation Mono",
    monospace;
}

.execution-log-path {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: #eff8f7;
}

.execution-log-elapsed {
  color: #88a5a1;
  font-size: 11px;
}

.execution-log-phases {
  grid-column: 2;
  list-style: none;
  margin: 4px 0 0;
  padding: 0;
  display: grid;
  gap: 4px;
}

.execution-log-phase {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  color: #b8c9c6;
  font-size: 11px;
}

.execution-log-phase-copy {
  display: grid;
  gap: 2px;
}

.execution-log-phase-copy small {
  color: #88a5a1;
  white-space: pre-wrap;
  word-break: break-word;
}

.execution-log-phase--error {
  color: #fca5a5;
}
</style>
