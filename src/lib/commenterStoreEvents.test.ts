import assert from 'node:assert/strict';

import { mergeRunDetailEvents } from './commenterStore';
import type { CommenterEventPayload, CommenterRunDetail } from './commenterTypes';

function event(
  kind: CommenterEventPayload['kind'],
  run_key: string,
  relative_path: string | null,
  created_at: number,
  message = ''
): CommenterEventPayload {
  return {
    kind,
    run_key,
    relative_path,
    level: kind === 'job_failed' ? 'error' : 'info',
    message,
    created_at
  };
}

function detail(run_key: string, events: CommenterEventPayload[]): CommenterRunDetail {
  return {
    run: {
      run_key,
      profile_key: 'profile-a',
      status: 'running',
      requested_by: null,
      run_mode: 'auto',
      total_jobs: 1,
      completed_jobs: 0,
      review_needed_jobs: 0,
      failed_jobs: 0,
      skipped_jobs: 0,
      pending_jobs: 1,
      current_file: 'src/a.ts',
      max_workers: 1,
      max_retries: 0,
      max_files: 1,
      allow_light_rewrite: true,
      json_handling_strategy: 'sidecar_only',
      created_at: 1,
      updated_at: 1,
      started_at: 1,
      finished_at: null
    },
    jobs: [
      {
        id: 1,
        relative_path: 'src/a.ts',
        status: 'requesting',
        language_hint: 'ts',
        write_strategy: 'annotate_in_place',
        retry_count: 0,
        error_message: null,
        before_artifact_path: null,
        candidate_artifact_path: null,
        sidecar_artifact_path: null
      }
    ],
    events
  };
}

{
  const stale_backend_detail = detail('run-1', [
    event('job_updated', 'run-1', 'src/a.ts', 10, 'Processing file: src/a.ts')
  ]);
  const live_detail = detail('run-1', [
    event('job_updated', 'run-1', 'src/a.ts', 10, 'Processing file: src/a.ts'),
    event(
      'request_started',
      'run-1',
      'src/a.ts',
      20,
      'AI request started -> https://example.test/v2/chat/completions; request artifact: request/src/a.ts.request.json'
    )
  ]);
  const execution_logs = [
    event('stream_chunk', 'run-1', 'src/a.ts', 25, 'partial response'),
    event(
      'model_response_completed',
      'run-1',
      'src/a.ts',
      30,
      'AI response completed: 12 characters (HTTP 200); response artifact: response/src/a.ts.response.json'
    ),
    event('request_started', 'other-run', 'src/a.ts', 40, 'other run should not merge')
  ];

  const merged = mergeRunDetailEvents(stale_backend_detail, live_detail, execution_logs);

  assert.deepEqual(
    merged.map((entry) => entry.kind),
    ['job_updated', 'request_started', 'model_response_completed'],
    'refreshing a stale backend detail should keep live request lifecycle events for the selected run'
  );
  assert.equal(
    merged.some((entry) => entry.kind === 'stream_chunk'),
    false,
    'stream chunks should stay out of selected run detail events'
  );
}

console.log('commenter store events PASSED');
