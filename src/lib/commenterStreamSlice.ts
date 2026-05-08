import type { CommenterEventPayload } from './commenterTypes';

export const STREAM_SLICE_TEXT_KEEP_COUNT = 30;
export const STREAM_SLICE_TEXT_CAP_BYTES = 5 * 1024 * 1024;
const TRUNCATION_MARKER = '\n[... truncated, full text in candidate.txt]\n';

export interface LiveStreamSlice {
  text: string;
  started_at: number;
  last_chunk_at: number;
  status: 'streaming' | 'completed' | 'failed';
  error: string | null;
  truncated: boolean;
}

export function streamSliceKey(run_key: string, relative_path: string): string {
  return `${run_key}|${relative_path}`;
}

function emptySlice(at: number): LiveStreamSlice {
  return {
    text: '',
    started_at: at,
    last_chunk_at: at,
    status: 'streaming',
    error: null,
    truncated: false
  };
}

function applyChunk(slice: LiveStreamSlice, message: string, at: number): LiveStreamSlice {
  if (slice.truncated) {
    return { ...slice, last_chunk_at: at };
  }
  const remaining = STREAM_SLICE_TEXT_CAP_BYTES - slice.text.length;
  if (remaining <= 0) {
    return {
      ...slice,
      text: slice.text + TRUNCATION_MARKER,
      truncated: true,
      last_chunk_at: at
    };
  }
  if (message.length <= remaining) {
    return { ...slice, text: slice.text + message, last_chunk_at: at };
  }
  return {
    ...slice,
    text: slice.text + message.slice(0, remaining) + TRUNCATION_MARKER,
    truncated: true,
    last_chunk_at: at
  };
}

function evictOldestText(slices: Map<string, LiveStreamSlice>): Map<string, LiveStreamSlice> {
  const withText = [...slices.entries()].filter(([, slice]) => slice.text.length > 0);
  if (withText.length <= STREAM_SLICE_TEXT_KEEP_COUNT) return slices;

  withText.sort((a, b) => a[1].last_chunk_at - b[1].last_chunk_at);
  const drop_count = withText.length - STREAM_SLICE_TEXT_KEEP_COUNT;
  const next = new Map(slices);
  for (let i = 0; i < drop_count; i += 1) {
    const [k, slice] = withText[i];
    next.set(k, { ...slice, text: '', truncated: false });
  }
  return next;
}

export function applyEventToStreamSlices(
  current: Map<string, LiveStreamSlice>,
  event: CommenterEventPayload
): Map<string, LiveStreamSlice> {
  if (!event.relative_path) return current;
  const k = streamSliceKey(event.run_key, event.relative_path);
  const next = new Map(current);

  switch (event.kind) {
    case 'request_started':
      next.set(k, emptySlice(event.created_at));
      return evictOldestText(next);
    case 'stream_chunk': {
      const existing = next.get(k) ?? emptySlice(event.created_at);
      next.set(k, applyChunk(existing, event.message, event.created_at));
      return evictOldestText(next);
    }
    case 'model_response_completed': {
      const existing = next.get(k);
      if (!existing) return current;
      next.set(k, { ...existing, status: 'completed', last_chunk_at: event.created_at });
      return next;
    }
    case 'job_failed': {
      const existing = next.get(k) ?? emptySlice(event.created_at);
      next.set(k, {
        ...existing,
        status: 'failed',
        error: event.message || existing.error,
        last_chunk_at: event.created_at
      });
      return next;
    }
    default:
      return current;
  }
}

export function rebuildStreamSlices(events: CommenterEventPayload[]): Map<string, LiveStreamSlice> {
  return [...events]
    .sort((left, right) => left.created_at - right.created_at)
    .reduce(
      (current, event) => applyEventToStreamSlices(current, event),
      new Map<string, LiveStreamSlice>()
    );
}
