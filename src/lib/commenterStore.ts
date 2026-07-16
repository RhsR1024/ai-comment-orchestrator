import { markRaw, reactive } from 'vue';

import {
  applyEventToStreamSlices,
  applyEventsToStreamSlices,
  rebuildStreamSlices,
  settleStreamSlicesForTerminalRun,
  type LiveStreamSlice
} from './commenterStreamSlice';
import type {
  CommenterDataPaths,
  CommenterDiffToolSettings,
  CommenterEnqueueRunRequest,
  CommenterEventPayload,
  CommenterProjectProfileDraft,
  CommenterProjectProfileView,
  CommenterReviewActionRequest,
  CommenterRunDetail,
  CommenterRunRecord,
  CommenterRunSettingsView,
  CommenterJobRecord
} from './commenterTypes';
import { is_run_finished } from './commenterView';
import { commenterApi, subscribeCommenterEvents } from './tauri';

interface CommenterStoreState {
  profiles: CommenterProjectProfileView[];
  runs: CommenterRunRecord[];
  selected_run_key: string | null;
  selected_run_detail: CommenterRunDetail | null;
  execution_logs: CommenterEventPayload[];
  live_streams: Map<string, LiveStreamSlice>;
  review_jobs: CommenterJobRecord[];
  history_runs: CommenterRunRecord[];
  app_settings: CommenterRunSettingsView | null;
  diff_tool_settings: CommenterDiffToolSettings | null;
  data_paths: CommenterDataPaths | null;
  loading: boolean;
  error_message: string | null;
}

const REFRESH_INTERVAL_MS = 3000;
const EXECUTION_LOG_CAP = 400;

const state = reactive<CommenterStoreState>({
  profiles: [],
  runs: [],
  selected_run_key: null,
  selected_run_detail: null,
  execution_logs: [],
  live_streams: new Map(),
  review_jobs: [],
  history_runs: [],
  app_settings: null,
  diff_tool_settings: null,
  data_paths: null,
  loading: false,
  error_message: null
});

let event_unsubscribe: (() => void) | null = null;
let event_subscription_pending = false;
let initialization_pending: Promise<void> | null = null;
let initialized = false;
let refresh_timer: number | null = null;
let runtime_refresh_pending: Promise<void> | null = null;
let active_start_requests = 0;
let chunk_buffer: CommenterEventPayload[] = [];
let chunk_flush_scheduled = false;

function eventKey(event: CommenterEventPayload): string {
  return [
    event.kind,
    event.run_key,
    event.relative_path ?? '',
    event.created_at,
    event.message
  ].join('|');
}

function rawDetail(detail: CommenterRunDetail | null): CommenterRunDetail | null {
  if (!detail) {
    return null;
  }
  const filtered_events = detail.events.filter((event) => event.kind !== 'stream_chunk');
  return markRaw({
    ...detail,
    events: filtered_events
  });
}

export function mergeRunDetailEvents(
  nextDetail: CommenterRunDetail,
  currentDetail: CommenterRunDetail | null,
  executionLogs: CommenterEventPayload[]
): CommenterEventPayload[] {
  const run_key = nextDetail.run.run_key;
  const merged = new Map<string, CommenterEventPayload>();
  const current_events = currentDetail?.run.run_key === run_key ? currentDetail.events : [];

  for (const event of [...nextDetail.events, ...current_events, ...executionLogs]) {
    if (event.run_key !== run_key || event.kind === 'stream_chunk') {
      continue;
    }
    merged.set(eventKey(event), event);
  }

  return [...merged.values()].sort((left, right) => left.created_at - right.created_at);
}

function mergeLiveStreamSnapshots(
  rebuilt: Map<string, LiveStreamSlice>,
  current: Map<string, LiveStreamSlice>,
  run_key: string
): Map<string, LiveStreamSlice> {
  const next = new Map(rebuilt);
  const run_prefix = `${run_key}|`;
  for (const [key, slice] of current) {
    if (!key.startsWith(run_prefix)) {
      continue;
    }
    const rebuilt_slice = next.get(key);
    if (!rebuilt_slice || slice.last_chunk_at > rebuilt_slice.last_chunk_at) {
      next.set(key, slice);
    }
  }
  return next;
}

function appendExecutionLog(event: CommenterEventPayload) {
  if (event.kind === 'stream_chunk') {
    return;
  }
  const key = eventKey(event);
  const existing_index = state.execution_logs.findIndex((entry) => eventKey(entry) === key);
  if (existing_index >= 0) {
    return;
  }
  let position = 0;
  while (
    position < state.execution_logs.length &&
    state.execution_logs[position].created_at >= event.created_at
  ) {
    position += 1;
  }
  state.execution_logs.splice(position, 0, event);
  if (state.execution_logs.length > EXECUTION_LOG_CAP) {
    state.execution_logs.length = EXECUTION_LOG_CAP;
  }
}

function mergeInitialExecutionLogs(events: CommenterEventPayload[]) {
  if (events.length === 0) {
    state.execution_logs = [];
    return;
  }
  const filtered = events.filter((event) => event.kind !== 'stream_chunk');
  filtered.sort((left, right) => right.created_at - left.created_at);
  state.execution_logs = filtered.slice(0, EXECUTION_LOG_CAP);
}

function recordDetailEvent(event: CommenterEventPayload) {
  const detail = state.selected_run_detail;
  if (!detail || detail.run.run_key !== event.run_key) {
    return;
  }
  if (event.kind === 'stream_chunk') {
    return;
  }
  state.selected_run_detail = markRaw({
    ...detail,
    events: [...detail.events, event]
  });
}

function flushChunkBuffer() {
  chunk_flush_scheduled = false;
  if (chunk_buffer.length === 0) {
    return;
  }
  const events = chunk_buffer;
  chunk_buffer = [];
  const next = applyEventsToStreamSlices(state.live_streams, events);
  if (next !== state.live_streams) {
    state.live_streams = next;
  }
}

function scheduleChunkFlush() {
  if (chunk_flush_scheduled) {
    return;
  }
  chunk_flush_scheduled = true;
  if (typeof window !== 'undefined' && typeof window.requestAnimationFrame === 'function') {
    window.requestAnimationFrame(flushChunkBuffer);
  } else {
    queueMicrotask(flushChunkBuffer);
  }
}

function applyLiveStreamEvent(event: CommenterEventPayload) {
  if (event.kind === 'stream_chunk') {
    chunk_buffer.push(event);
    scheduleChunkFlush();
    return;
  }
  const next = applyEventToStreamSlices(state.live_streams, event);
  if (next !== state.live_streams) {
    state.live_streams = next;
  }
}

function appendExecutionEvent(event: CommenterEventPayload) {
  appendExecutionLog(event);
  recordDetailEvent(event);
  applyLiveStreamEvent(event);
}

function applySelectedRunDetail(detail: CommenterRunDetail | null) {
  if (!detail) {
    state.selected_run_detail = null;
    mergeInitialExecutionLogs([]);
    state.live_streams = new Map();
    return;
  }

  const merged_events = mergeRunDetailEvents(detail, state.selected_run_detail, state.execution_logs);
  state.selected_run_detail = rawDetail({
    ...detail,
    events: merged_events
  });
  mergeInitialExecutionLogs(merged_events);
  const merged_streams = mergeLiveStreamSnapshots(
    rebuildStreamSlices(detail.events),
    state.live_streams,
    detail.run.run_key
  );
  state.live_streams = is_run_finished(detail.run.status)
    ? settleStreamSlicesForTerminalRun(
        merged_streams,
        detail.run.run_key,
        detail.run.finished_at ?? detail.run.updated_at,
        detail.jobs
      )
    : merged_streams;
}

function hasActiveRun(): boolean {
  return (
    active_start_requests > 0 ||
    state.runs.some((run) => ['running', 'pausing', 'scanning'].includes(run.status))
  );
}

function stopAutoRefreshIfIdle() {
  if (refresh_timer && !hasActiveRun()) {
    clearInterval(refresh_timer);
    refresh_timer = null;
  }
}

function startAutoRefresh() {
  if (refresh_timer || typeof window === 'undefined') {
    return;
  }

  refresh_timer = window.setInterval(() => {
    void refreshRuntimeCollectionsQuiet();
  }, REFRESH_INTERVAL_MS);
}

async function ensureEventSubscription() {
  if (event_unsubscribe || event_subscription_pending) {
    return;
  }

  event_subscription_pending = true;
  try {
    event_unsubscribe = await subscribeCommenterEvents((payload) => {
      appendExecutionEvent(payload);
      if (['run_started', 'request_started', 'job_updated'].includes(payload.kind)) {
        startAutoRefresh();
      }
    });
  } finally {
    event_subscription_pending = false;
  }
}

async function refreshCollections() {
  const [profiles, runs, review_jobs, app_settings, diff_tool_settings, data_paths] = await Promise.all([
    commenterApi.listProjectProfiles(),
    commenterApi.listRuns(),
    commenterApi.listReviewJobs(),
    commenterApi.getAppSettings(),
    commenterApi.getDiffToolSettings(),
    state.data_paths ? Promise.resolve(state.data_paths) : commenterApi.getDataPaths()
  ]);

  state.profiles = profiles;
  state.runs = runs;
  state.review_jobs = review_jobs;
  state.history_runs = runs.filter((run) => is_run_finished(run.status));
  state.app_settings = app_settings;
  state.diff_tool_settings = diff_tool_settings;
  state.data_paths = data_paths;

  if (!state.selected_run_key && runs.length > 0) {
    state.selected_run_key = runs[0].run_key;
  }

  if (state.selected_run_key) {
    applySelectedRunDetail(await commenterApi.getRunDetail(state.selected_run_key));
  } else {
    applySelectedRunDetail(null);
  }
  stopAutoRefreshIfIdle();
}

async function refreshRuntimeCollections() {
  const [runs, review_jobs] = await Promise.all([
    commenterApi.listRuns(),
    commenterApi.listReviewJobs()
  ]);

  state.runs = runs;
  state.review_jobs = review_jobs;
  state.history_runs = runs.filter((run) => is_run_finished(run.status));

  if (!state.selected_run_key && runs.length > 0) {
    state.selected_run_key = runs[0].run_key;
  }
  if (state.selected_run_key) {
    applySelectedRunDetail(await commenterApi.getRunDetail(state.selected_run_key));
  } else {
    applySelectedRunDetail(null);
  }
  stopAutoRefreshIfIdle();
}

function refreshRuntimeCollectionsQuiet(): Promise<void> {
  if (runtime_refresh_pending) {
    return runtime_refresh_pending;
  }
  runtime_refresh_pending = refreshRuntimeCollections()
    .catch((error) => {
      state.error_message = error instanceof Error ? error.message : String(error);
    })
    .finally(() => {
      runtime_refresh_pending = null;
    });
  return runtime_refresh_pending;
}

async function runWithRefresh<T>(work: () => Promise<T>): Promise<T> {
  state.loading = true;
  state.error_message = null;
  try {
    const result = await work();
    await refreshCollections();
    return result;
  } catch (error) {
    state.error_message = error instanceof Error ? error.message : String(error);
    throw error;
  } finally {
    state.loading = false;
  }
}

export const commenterStore = {
  state,
  async initialize() {
    await ensureEventSubscription();
    if (initialized) {
      return;
    }
    if (initialization_pending) {
      return initialization_pending;
    }
    initialization_pending = runWithRefresh(async () => undefined)
      .then(() => {
        initialized = true;
      })
      .finally(() => {
        initialization_pending = null;
      });
    return initialization_pending;
  },
  async refresh() {
    return runWithRefresh(async () => undefined);
  },
  selectRun(run_key: string) {
    state.selected_run_key = run_key;
    state.live_streams = new Map();
    return runWithRefresh(async () => undefined);
  },
  async saveProfile(request: CommenterProjectProfileDraft) {
    return runWithRefresh(() => commenterApi.upsertProjectProfile(request));
  },
  async deleteProfile(project_key: string) {
    return runWithRefresh(() => commenterApi.deleteProjectProfile(project_key));
  },
  async enqueueRun(request: CommenterEnqueueRunRequest) {
    return runWithRefresh(async () => {
      const run = await commenterApi.enqueueRun(request);
      state.selected_run_key = run.run_key;
      return run;
    });
  },
  async startRun(run_key: string) {
    state.error_message = null;
    state.selected_run_key = run_key;
    const timestamp = Date.now();
    const run = state.runs.find((entry) => entry.run_key === run_key);
    if (run) {
      run.status = 'running';
      run.started_at ??= timestamp;
      run.updated_at = timestamp;
    }
    appendExecutionEvent({
      kind: 'run_started',
      run_key,
      relative_path: null,
      level: 'info',
      message: `Start command sent for ${run_key}`,
      created_at: timestamp
    });
    startAutoRefresh();
    active_start_requests += 1;

    void commenterApi
      .startRun(run_key)
      .then((detail) => {
        state.selected_run_key = detail.run.run_key;
        applySelectedRunDetail(detail);
        return refreshRuntimeCollectionsQuiet();
      })
      .catch((error) => {
        state.error_message = error instanceof Error ? error.message : String(error);
      })
      .finally(() => {
        active_start_requests = Math.max(0, active_start_requests - 1);
        stopAutoRefreshIfIdle();
      });
  },
  async deleteRun(run_key: string) {
    return runWithRefresh(async () => {
      const deleted = await commenterApi.deleteRun(run_key);
      state.execution_logs = state.execution_logs.filter((event) => event.run_key !== run_key);
      const next_streams = new Map(state.live_streams);
      for (const key of [...next_streams.keys()]) {
        if (key.startsWith(`${run_key}|`)) {
          next_streams.delete(key);
        }
      }
      state.live_streams = next_streams;
      if (state.selected_run_key === run_key) {
        state.selected_run_key = null;
        applySelectedRunDetail(null);
      }
      return deleted;
    });
  },
  async pauseRun(run_key: string) {
    return runWithRefresh(() => commenterApi.pauseRun(run_key));
  },
  async resumeRun(run_key: string) {
    return runWithRefresh(() => commenterApi.resumeRun(run_key));
  },
  async cancelRun(run_key: string) {
    return runWithRefresh(() => commenterApi.cancelRun(run_key));
  },
  async acceptReviewJob(request: CommenterReviewActionRequest) {
    return runWithRefresh(() => commenterApi.acceptReviewJob(request));
  },
  async rejectReviewJob(request: CommenterReviewActionRequest) {
    return runWithRefresh(() => commenterApi.rejectReviewJob(request));
  },
  async retryJob(request: CommenterReviewActionRequest) {
    return runWithRefresh(() => commenterApi.retryJob(request));
  },
  async openExternalDiff(request: CommenterReviewActionRequest) {
    return runWithRefresh(async () => {
      await commenterApi.openExternalDiff(request);
    });
  },
  async rollbackRun(run_key: string) {
    return runWithRefresh(() => commenterApi.rollbackRun(run_key));
  },
  async saveDiffToolSettings(request: CommenterDiffToolSettings) {
    return runWithRefresh(() => commenterApi.updateDiffToolSettings(request));
  },
  async saveAppSettings(request: CommenterRunSettingsView) {
    return runWithRefresh(() => commenterApi.updateAppSettings(request));
  }
};
