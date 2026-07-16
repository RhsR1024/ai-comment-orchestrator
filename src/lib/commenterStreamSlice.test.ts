import assert from 'node:assert/strict';

import {
  applyEventToStreamSlices,
  applyEventsToStreamSlices,
  rebuildStreamSlices,
  settleStreamSlicesForTerminalRun,
  STREAM_SLICE_TEXT_CAP_BYTES,
  STREAM_SLICE_TEXT_KEEP_COUNT,
  type LiveStreamSlice
} from './commenterStreamSlice';
import type { CommenterEventPayload, CommenterJobRecord } from './commenterTypes';

function event(
  kind: CommenterEventPayload['kind'],
  relative_path: string,
  at: number,
  message = ''
): CommenterEventPayload {
  return {
    kind,
    run_key: 'r1',
    relative_path,
    level: kind === 'job_failed' ? 'error' : 'info',
    message,
    created_at: at
  };
}

function key(path: string): string {
  return `r1|${path}`;
}

function job(relative_path: string, status: CommenterJobRecord['status'], error_message: string | null = null): CommenterJobRecord {
  return {
    id: 1,
    relative_path,
    status,
    language_hint: null,
    write_strategy: 'annotate_in_place',
    retry_count: 0,
    error_message,
    before_artifact_path: null,
    candidate_artifact_path: null,
    sidecar_artifact_path: null
  };
}

// 1. request_started creates an empty slice in 'streaming'
{
  const next = applyEventToStreamSlices(new Map(), event('request_started', 'a.ts', 10));
  const slice = next.get(key('a.ts'))!;
  assert.equal(slice.text, '');
  assert.equal(slice.status, 'streaming');
  assert.equal(slice.started_at, 10);
}

// 2. stream_chunk events concatenate in order
{
  let map = new Map<string, LiveStreamSlice>();
  map = applyEventToStreamSlices(map, event('request_started', 'a.ts', 10));
  map = applyEventToStreamSlices(map, event('stream_chunk', 'a.ts', 20, 'hello '));
  map = applyEventToStreamSlices(map, event('stream_chunk', 'a.ts', 30, 'world'));
  assert.equal(map.get(key('a.ts'))!.text, 'hello world');
  assert.equal(map.get(key('a.ts'))!.last_chunk_at, 30);
}

// 3. model_response_completed flips status without touching text
{
  let map = new Map<string, LiveStreamSlice>();
  map = applyEventToStreamSlices(map, event('request_started', 'a.ts', 10));
  map = applyEventToStreamSlices(map, event('stream_chunk', 'a.ts', 20, 'hi'));
  map = applyEventToStreamSlices(map, event('model_response_completed', 'a.ts', 30));
  const slice = map.get(key('a.ts'))!;
  assert.equal(slice.text, 'hi');
  assert.equal(slice.status, 'completed');
}

// 4. job_failed sets status='failed' and error
{
  let map = new Map<string, LiveStreamSlice>();
  map = applyEventToStreamSlices(map, event('request_started', 'a.ts', 10));
  map = applyEventToStreamSlices(map, event('job_failed', 'a.ts', 20, 'boom'));
  const slice = map.get(key('a.ts'))!;
  assert.equal(slice.status, 'failed');
  assert.equal(slice.error, 'boom');
}

// 5. A stream beyond the bounded live-file window evicts the oldest text.
{
  let map = new Map<string, LiveStreamSlice>();
  for (let i = 0; i < STREAM_SLICE_TEXT_KEEP_COUNT + 1; i += 1) {
    const path = `f${i}.ts`;
    map = applyEventToStreamSlices(map, event('request_started', path, 100 + i));
    map = applyEventToStreamSlices(map, event('stream_chunk', path, 200 + i, 'data'));
  }
  const oldest = map.get(key('f0.ts'))!;
  assert.equal(oldest.text, '', 'oldest slice should have its text cleared');
  assert.equal(oldest.status, 'streaming', 'metadata is retained');
  const newest = map.get(key(`f${STREAM_SLICE_TEXT_KEEP_COUNT}.ts`))!;
  assert.equal(newest.text, 'data');
}

// 6. The bounded per-file preview stops appending and inserts a truncation marker.
{
  let map = new Map<string, LiveStreamSlice>();
  map = applyEventToStreamSlices(map, event('request_started', 'big.ts', 10));
  const big = 'x'.repeat(STREAM_SLICE_TEXT_CAP_BYTES);
  map = applyEventToStreamSlices(map, event('stream_chunk', 'big.ts', 20, big));
  map = applyEventToStreamSlices(map, event('stream_chunk', 'big.ts', 30, 'extra'));
  const slice = map.get(key('big.ts'))!;
  assert.ok(slice.text.includes('truncated'), 'should contain truncation marker');
  assert.ok(!slice.text.includes('extra'), 'further chunks must be dropped');
}

// 7. rebuilding from persisted events restores stream history after reload
{
  const map = rebuildStreamSlices([
    event('model_response_completed', 'b.ts', 40),
    event('stream_chunk', 'b.ts', 30, 'world'),
    event('request_started', 'b.ts', 10),
    event('stream_chunk', 'b.ts', 20, 'hello ')
  ]);
  const slice = map.get(key('b.ts'))!;
  assert.equal(slice.text, 'hello world');
  assert.equal(slice.status, 'completed');
}

// Batched chunks use one state transition while preserving exact ordering.
{
  const initial = applyEventToStreamSlices(new Map(), event('request_started', 'batch.ts', 10));
  const next = applyEventsToStreamSlices(initial, [
    event('stream_chunk', 'batch.ts', 20, 'one '),
    event('stream_chunk', 'batch.ts', 30, 'two'),
    event('model_response_completed', 'batch.ts', 40)
  ]);
  const slice = next.get(key('batch.ts'))!;
  assert.equal(slice.text, 'one two');
  assert.equal(slice.status, 'completed');
  assert.notEqual(next, initial);
}

// Terminal run refreshes settle legacy CodeBuddy slices whose terminal event omitted a path.
{
  let map = new Map<string, LiveStreamSlice>();
  map = applyEventToStreamSlices(map, event('request_started', 'done.ts', 10));
  map = applyEventToStreamSlices(map, event('stream_chunk', 'done.ts', 20, 'no changes'));
  map = applyEventToStreamSlices(map, event('request_started', 'failed.ts', 11));
  map = settleStreamSlicesForTerminalRun(map, 'r1', 50, [
    job('done.ts', 'skipped'),
    job('failed.ts', 'failed', 'agent failed')
  ]);
  assert.equal(map.get(key('done.ts'))!.status, 'completed');
  assert.equal(map.get(key('failed.ts'))!.status, 'failed');
  assert.equal(map.get(key('failed.ts'))!.error, 'agent failed');
}

console.log('commenter stream slice PASSED');
