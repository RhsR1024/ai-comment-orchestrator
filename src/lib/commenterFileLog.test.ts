import assert from 'node:assert/strict';

import { buildFileLogEntries } from './commenterFileLog';
import type { CommenterEventPayload, CommenterJobRecord } from './commenterTypes';

function job(relative_path: string, status: CommenterJobRecord['status']): CommenterJobRecord {
  return {
    id: 0,
    relative_path,
    status,
    language_hint: null,
    write_strategy: 'auto',
    retry_count: 0,
    error_message: null,
    before_artifact_path: null,
    candidate_artifact_path: null,
    sidecar_artifact_path: null
  };
}

function event(
  kind: CommenterEventPayload['kind'],
  relative_path: string | null,
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

// 1. Empty inputs return []
assert.deepEqual(buildFileLogEntries([], []), []);

// 2. Single file with full phase sequence keeps phases in order
{
  const jobs = [job('src/a.ts', 'done')];
  const events = [
    event('request_started', 'src/a.ts', 100),
    event('stream_chunk', 'src/a.ts', 110, 'hello '),
    event('stream_chunk', 'src/a.ts', 120, 'world'),
    event('model_response_completed', 'src/a.ts', 130),
    event('job_updated', 'src/a.ts', 140)
  ];
  const result = buildFileLogEntries(jobs, events);
  assert.equal(result.length, 1);
  assert.equal(result[0].relative_path, 'src/a.ts');
  assert.equal(result[0].status, 'done');
  assert.deepEqual(result[0].phases.map((p) => p.phase), [
    'requested',
    'first_chunk',
    'response_done',
    'written'
  ]);
  assert.equal(result[0].started_at, 100);
  assert.equal(result[0].ended_at, 140);
}

// 3. Interleaved events for multiple files group correctly
{
  const jobs = [job('src/a.ts', 'done'), job('src/b.ts', 'failed')];
  const events = [
    event('request_started', 'src/a.ts', 100),
    event('request_started', 'src/b.ts', 105),
    event('stream_chunk', 'src/a.ts', 120),
    event('job_failed', 'src/b.ts', 130, 'boom')
  ];
  const result = buildFileLogEntries(jobs, events);
  assert.equal(result.find((e) => e.relative_path === 'src/b.ts')?.error_message, 'boom');
  assert.equal(result.find((e) => e.relative_path === 'src/a.ts')?.phases.length, 2);
}

// 4. job_failed sets error_message to most recent failure
{
  const jobs = [job('src/x.ts', 'failed')];
  const events = [
    event('job_failed', 'src/x.ts', 100, 'first'),
    event('job_failed', 'src/x.ts', 200, 'second')
  ];
  const result = buildFileLogEntries(jobs, events);
  assert.equal(result[0].error_message, 'second');
}

// 5. Orphan event (path not in jobs) is silently dropped
{
  const result = buildFileLogEntries(
    [job('src/a.ts', 'done')],
    [
      event('request_started', 'src/a.ts', 100),
      event('request_started', 'src/ghost.ts', 110)
    ]
  );
  assert.equal(result.length, 1);
  assert.equal(result[0].relative_path, 'src/a.ts');
}

// 6. phase messages are retained for request/response debugging
{
  const result = buildFileLogEntries(
    [job('src/a.ts', 'done')],
    [
      event('request_started', 'src/a.ts', 100, 'POST /v2/chat/completions request artifact saved'),
      event('model_response_completed', 'src/a.ts', 120, 'HTTP 200 response artifact saved')
    ]
  );
  assert.equal(result[0].phases[0].message, 'POST /v2/chat/completions request artifact saved');
  assert.equal(result[0].phases[1].message, 'HTTP 200 response artifact saved');
}

console.log('commenter file log PASSED');
