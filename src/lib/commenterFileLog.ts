import type {
  CommenterEventKind,
  CommenterEventLevel,
  CommenterEventPayload,
  CommenterJobRecord,
  CommenterJobStatus
} from './commenterTypes';

export type PhaseTag =
  | 'queued'
  | 'requested'
  | 'first_chunk'
  | 'response_done'
  | 'validated'
  | 'written'
  | 'review_requested'
  | 'failed'
  | 'rolled_back';

export interface FileLogEntry {
  relative_path: string;
  status: CommenterJobStatus;
  phases: { phase: PhaseTag; at: number; level: CommenterEventLevel; message: string }[];
  started_at: number | null;
  ended_at: number | null;
  error_message: string | null;
}

const KIND_TO_PHASE: Partial<Record<CommenterEventKind, PhaseTag>> = {
  run_queued: 'queued',
  request_started: 'requested',
  stream_chunk: 'first_chunk',
  model_response_completed: 'response_done',
  job_updated: 'written',
  review_requested: 'review_requested',
  job_failed: 'failed',
  run_rolled_back: 'rolled_back'
};

export function buildFileLogEntries(
  jobs: CommenterJobRecord[],
  events: CommenterEventPayload[]
): FileLogEntry[] {
  const known = new Map<string, FileLogEntry>();
  for (const job of jobs) {
    known.set(job.relative_path, {
      relative_path: job.relative_path,
      status: job.status,
      phases: [],
      started_at: null,
      ended_at: null,
      error_message: null
    });
  }

  const sorted = [...events].sort((a, b) => a.created_at - b.created_at);
  const seen_first_chunk = new Set<string>();

  for (const event of sorted) {
    if (!event.relative_path) continue;
    const entry = known.get(event.relative_path);
    if (!entry) continue;

    const phase = KIND_TO_PHASE[event.kind];
    if (!phase) continue;

    if (phase === 'first_chunk') {
      if (seen_first_chunk.has(event.relative_path)) continue;
      seen_first_chunk.add(event.relative_path);
    }

    entry.phases.push({
      phase,
      at: event.created_at,
      level: event.level,
      message: event.message
    });
    if (entry.started_at === null) entry.started_at = event.created_at;
    entry.ended_at = event.created_at;

    if (event.kind === 'job_failed') {
      entry.error_message = event.message || entry.error_message;
    }
  }

  return jobs.map((job) => known.get(job.relative_path)!).filter(Boolean);
}
