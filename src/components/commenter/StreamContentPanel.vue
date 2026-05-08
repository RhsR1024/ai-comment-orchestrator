<script setup lang="ts">
import { computed, ref, watch } from 'vue';

import { commenterStore } from '../../lib/commenterStore';
import { commenterApi } from '../../lib/tauri';
import type { CommenterEventPayload, CommenterJobStatus } from '../../lib/commenterTypes';
import { use_messages } from '../../locales/messages';

const props = defineProps<{
  mode: 'live' | 'locked';
  run_key: string | null;
  relative_path: string | null;
  live_text: string;
  streamLastChunkAt: number | null;
  status: CommenterJobStatus | 'streaming' | 'idle';
  error_message: string | null;
}>();

const { t } = use_messages();

const body_ref = ref<HTMLElement | null>(null);
const fallback_text = ref('');
const fetch_error = ref<string | null>(null);
const fetching = ref(false);
const active_tab = ref<'diff' | 'stream' | 'original' | 'request'>('stream');

const file_events = computed<CommenterEventPayload[]>(() => {
  const path = props.relative_path;
  const detail = commenterStore.state.selected_run_detail;
  if (!path || !detail) {
    return [];
  }
  return detail.events.filter((event) => event.relative_path === path);
});

const REQUEST_URL_PATTERN = /https?:\/\/[^\s;]+/i;
const ARTIFACT_PATTERN = /artifact:\s*([^\s;]+)/i;
const HTTP_STATUS_PATTERN = /HTTP\s+(\d{3})/i;
const CHARS_PATTERN = /(\d+)\s+characters/i;

function findEvent(kind: string): CommenterEventPayload | null {
  return file_events.value.find((event) => event.kind === kind) ?? null;
}

const request_started = computed(() => findEvent('request_started'));
const response_completed = computed(() => findEvent('model_response_completed'));
const job_failed = computed(() => findEvent('job_failed'));

const request_url = computed(() => {
  const message = request_started.value?.message ?? '';
  return REQUEST_URL_PATTERN.exec(message)?.[0] ?? null;
});

const request_artifact = computed(() => {
  const message = request_started.value?.message ?? '';
  return ARTIFACT_PATTERN.exec(message)?.[1] ?? null;
});

const response_artifact = computed(() => {
  const message = response_completed.value?.message ?? '';
  return ARTIFACT_PATTERN.exec(message)?.[1] ?? null;
});

const http_status = computed(() => {
  const message = response_completed.value?.message ?? job_failed.value?.message ?? '';
  return HTTP_STATUS_PATTERN.exec(message)?.[1] ?? null;
});

const response_chars = computed(() => {
  const message = response_completed.value?.message ?? '';
  return CHARS_PATTERN.exec(message)?.[1] ?? null;
});

const is_streaming_response = computed(
  () => !job_failed.value && !response_completed.value && (props.status === 'streaming' || props.live_text.length > 0)
);

const request_detail_events = computed<CommenterEventPayload[]>(() => {
  const events = [...file_events.value];
  if (
    is_streaming_response.value &&
    props.run_key &&
    props.relative_path &&
    props.streamLastChunkAt &&
    !events.some((event) => event.kind === 'stream_chunk')
  ) {
    events.push({
      kind: 'stream_chunk',
      run_key: props.run_key,
      relative_path: props.relative_path,
      level: 'info',
      message: '',
      created_at: props.streamLastChunkAt
    });
  }
  return events.sort((left, right) => left.created_at - right.created_at);
});

const result_label = computed(() => {
  if (job_failed.value) {
    return http_status.value ? `失败 · HTTP ${http_status.value}` : '失败';
  }
  if (response_completed.value) {
    return http_status.value ? `成功 · HTTP ${http_status.value}` : '成功';
  }
  if (is_streaming_response.value) {
    return t('commenter.request.streaming');
  }
  if (request_started.value) {
    return '请求中';
  }
  return '尚未发起请求';
});

const result_state = computed<'success' | 'failed' | 'streaming' | 'pending' | 'idle'>(() => {
  if (job_failed.value) return 'failed';
  if (response_completed.value) return 'success';
  if (is_streaming_response.value) return 'streaming';
  if (request_started.value) return 'pending';
  return 'idle';
});

function format_time(at: number): string {
  return new Date(at).toLocaleTimeString();
}

const display_text = computed(() => (props.live_text.length > 0 ? props.live_text : fallback_text.value));

const language_label = computed(() => {
  const path = props.relative_path ?? '';
  const ext = path.split('.').pop()?.toLowerCase() ?? '';
  const map: Record<string, string> = {
    ts: 'TypeScript', tsx: 'TSX', js: 'JavaScript', jsx: 'JSX',
    vue: 'Vue', go: 'Go', rs: 'Rust', py: 'Python', java: 'Java',
    css: 'CSS', html: 'HTML', md: 'Markdown', json: 'JSON', yaml: 'YAML', yml: 'YAML',
    sql: 'SQL', sh: 'Shell', toml: 'TOML', proto: 'Protobuf'
  };
  return map[ext] ?? ext.toUpperCase() ?? '';
});

const size_kb_label = computed(() => {
  const bytes = new TextEncoder().encode(display_text.value).length;
  return bytes > 0 ? `${(bytes / 1024).toFixed(1)} KB` : '';
});

const line_count_label = computed(() => {
  if (!display_text.value) return '';
  return `${display_text.value.split('\n').length} 行`;
});

const chunk_count_label = computed(() => {
  const slice = props.live_text ? Math.max(1, Math.ceil(props.live_text.length / 64)) : 0;
  return slice > 0 ? `${slice} chunks` : '';
});

const badge_label = computed(() => {
  if (!props.relative_path) {
    return '';
  }
  if (props.status === 'failed') {
    return t('commenter.stream.failed');
  }
  if (props.status === 'review_needed') {
    return t('commenter.stream.review');
  }
  if (props.status === 'done') {
    return t('commenter.stream.done');
  }
  return props.mode === 'live' ? t('commenter.stream.live') : t('commenter.stream.locked');
});

const badge_class = computed(() => {
  if (props.status === 'failed') {
    return 'stream-badge stream-badge--failed';
  }
  if (props.status === 'review_needed') {
    return 'stream-badge stream-badge--review';
  }
  if (props.status === 'done') {
    return 'stream-badge stream-badge--done';
  }
  return props.mode === 'live'
    ? 'stream-badge stream-badge--live'
    : 'stream-badge stream-badge--locked';
});

async function maybeFetchCandidate() {
  fallback_text.value = '';
  fetch_error.value = null;
  if (props.mode !== 'locked') {
    return;
  }
  if (!props.run_key || !props.relative_path) {
    return;
  }
  if (props.live_text.length > 0) {
    return;
  }
  if (props.status !== 'done' && props.status !== 'review_needed') {
    return;
  }

  fetching.value = true;
  try {
    fallback_text.value = await commenterApi.getCandidateText(props.run_key, props.relative_path);
  } catch (error) {
    fetch_error.value = error instanceof Error ? error.message : String(error);
  } finally {
    fetching.value = false;
  }
}

watch(
  () => [props.mode, props.run_key, props.relative_path, props.status, props.live_text.length] as const,
  () => {
    void maybeFetchCandidate();
  },
  { immediate: true }
);

watch(
  () => [props.mode, props.status, display_text.value] as const,
  async ([mode, status]) => {
    if (mode !== 'live' || status !== 'streaming') {
      return;
    }
    await Promise.resolve();
    if (body_ref.value) {
      body_ref.value.scrollTop = body_ref.value.scrollHeight;
    }
  }
);
</script>

<template>
  <section class="stream-content-panel">
    <header class="stream-header">
      <div class="stream-header-main">
        <span class="stream-path mono">{{ relative_path ?? t('commenter.stream.idle') }}</span>
        <span v-if="language_label" class="stream-lang">{{ language_label }}</span>
        <span v-if="size_kb_label" class="stream-size">{{ size_kb_label }}</span>
        <span v-if="line_count_label" class="stream-lines">{{ line_count_label }}</span>
      </div>
      <span v-if="relative_path" :class="badge_class">{{ badge_label }}</span>
    </header>

    <nav class="stream-tabs" aria-label="流视图切换">
      <button type="button" :class="{ active: active_tab === 'diff' }" @click="active_tab = 'diff'">Diff</button>
      <button type="button" :class="{ active: active_tab === 'stream' }" @click="active_tab = 'stream'">{{ t('commenter.stream.response') }}</button>
      <button type="button" :class="{ active: active_tab === 'original' }" @click="active_tab = 'original'">{{ t('commenter.stream.original') }}</button>
      <button type="button" :class="{ active: active_tab === 'request' }" @click="active_tab = 'request'">{{ t('commenter.stream.requestDetail') }}</button>
    </nav>

    <div
      v-if="error_message"
      class="stream-error"
    >
      {{ error_message }}
    </div>

    <template v-if="active_tab === 'stream'">
      <pre
        v-if="display_text || fetching"
        ref="body_ref"
        class="stream-body mono"
      ><code>{{ display_text }}<span
        v-if="mode === 'live' && status === 'streaming'"
        class="stream-cursor"
      /></code></pre>

      <div
        v-else-if="fetch_error"
        class="stream-error"
      >
        {{ fetch_error }}
      </div>

      <div
        v-else
        class="empty-state"
      >
        {{ relative_path ? t('commenter.stream.empty') : t('commenter.stream.idle') }}
      </div>
    </template>

    <section v-else-if="active_tab === 'request'" class="request-detail">
      <div v-if="!relative_path" class="empty-state">
        {{ t('commenter.stream.idle') }}
      </div>
      <template v-else>
        <header class="request-detail-summary">
          <span class="request-detail-result" :data-state="result_state">{{ result_label }}</span>
          <span v-if="response_chars" class="request-detail-chars">{{ response_chars }} chars</span>
        </header>
        <dl class="request-detail-list">
          <div v-if="request_url">
            <dt>{{ t('commenter.request.endpoint') }}</dt>
            <dd class="mono">{{ request_url }}</dd>
          </div>
          <div v-if="http_status">
            <dt>{{ t('commenter.request.httpStatus') }}</dt>
            <dd class="mono">{{ http_status }}</dd>
          </div>
          <div v-if="request_artifact">
            <dt>{{ t('commenter.request.requestArtifact') }}</dt>
            <dd class="mono">{{ request_artifact }}</dd>
          </div>
          <div v-if="response_artifact">
            <dt>{{ t('commenter.request.responseArtifact') }}</dt>
            <dd class="mono">{{ response_artifact }}</dd>
          </div>
          <div v-if="job_failed?.message">
            <dt>{{ t('commenter.request.errorMessage') }}</dt>
            <dd class="mono request-detail-error">{{ job_failed.message }}</dd>
          </div>
        </dl>

        <h4 class="request-detail-section">{{ t('commenter.request.timeline') }}</h4>
        <ul v-if="request_detail_events.length > 0" class="request-detail-events">
          <li
            v-for="event in request_detail_events"
            :key="`${event.kind}-${event.created_at}`"
          >
            <span class="request-detail-time mono">{{ format_time(event.created_at) }}</span>
            <span class="request-detail-kind">{{ t(`commenter.event.${event.kind}`) }}</span>
            <span v-if="event.message" class="request-detail-message">{{ event.message }}</span>
          </li>
        </ul>
        <div v-else class="empty-state">
          {{ t('commenter.request.empty') }}
        </div>
      </template>
    </section>

    <div class="stream-meta">
      <span>UTF-8</span>
      <span>LF</span>
      <span v-if="chunk_count_label">{{ chunk_count_label }}</span>
    </div>
  </section>
</template>

<style scoped>
.stream-content-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.stream-badge {
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.04em;
}

.stream-badge--live {
  background: rgba(34, 197, 94, 0.18);
  color: var(--aco-green);
}

.stream-badge--locked {
  background: var(--aco-surface-3);
  color: var(--aco-muted);
}

.stream-badge--done {
  background: rgba(122, 162, 247, 0.18);
  color: var(--aco-blue);
}

.stream-badge--review {
  background: rgba(245, 185, 66, 0.18);
  color: var(--aco-yellow);
}

.stream-badge--failed {
  background: rgba(239, 90, 111, 0.18);
  color: var(--aco-red);
}

.stream-body {
  flex: 1;
  margin: 0;
  padding: 14px;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-word;
  font-size: 12px;
  line-height: 1.55;
  color: var(--aco-text);
}

.stream-cursor {
  display: inline-block;
  width: 7px;
  height: 1em;
  margin-left: 2px;
  vertical-align: text-bottom;
  background: currentColor;
  animation: stream-blink 1s step-start infinite;
}

@keyframes stream-blink {
  50% {
    opacity: 0;
  }
}

.stream-error {
  padding: 12px 14px;
  background: rgba(239, 90, 111, 0.12);
  color: var(--aco-red);
  border-bottom: 1px solid rgba(239, 90, 111, 0.18);
}

.request-detail {
  flex: 1;
  overflow: auto;
  padding: 16px 18px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  font-size: 12px;
  color: var(--aco-text);
}

.request-detail-summary {
  display: inline-flex;
  align-items: center;
  gap: 10px;
}

.request-detail-result {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  border: 1px solid var(--aco-border);
  border-radius: 999px;
  padding: 3px 10px;
  background: var(--aco-surface-2);
  color: var(--aco-muted);
  font-size: 12px;
}

.request-detail-result[data-state='success'] {
  color: var(--aco-green);
  border-color: rgba(52, 211, 153, 0.4);
}

.request-detail-result[data-state='failed'] {
  color: var(--aco-red);
  border-color: rgba(239, 90, 111, 0.4);
}

.request-detail-result[data-state='pending'] {
  color: var(--aco-yellow);
  border-color: rgba(245, 185, 66, 0.4);
}

.request-detail-result[data-state='streaming'] {
  color: var(--aco-green);
  border-color: rgba(52, 211, 153, 0.4);
}

.request-detail-chars {
  color: var(--aco-muted);
}

.request-detail-list {
  display: grid;
  gap: 6px;
  margin: 0;
}

.request-detail-list > div {
  display: grid;
  grid-template-columns: 110px 1fr;
  gap: 12px;
}

.request-detail-list dt {
  color: var(--aco-muted);
}

.request-detail-list dd {
  margin: 0;
  word-break: break-all;
}

.request-detail-error {
  color: var(--aco-red);
}

.request-detail-section {
  margin: 4px 0 0;
  font-size: 12px;
  color: var(--aco-muted);
  font-weight: 600;
}

.request-detail-events {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.request-detail-events li {
  display: grid;
  grid-template-columns: 80px 110px 1fr;
  gap: 10px;
  padding: 4px 0;
  border-bottom: 1px solid var(--aco-border);
  align-items: baseline;
}

.request-detail-time {
  color: var(--aco-subtle);
}

.request-detail-kind {
  color: var(--aco-text);
}

.request-detail-message {
  color: var(--aco-muted);
  word-break: break-word;
}
</style>
