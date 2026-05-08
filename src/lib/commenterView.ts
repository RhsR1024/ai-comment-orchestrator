import type {
  CommenterEventPayload,
  CommenterJobRecord,
  CommenterRunRecord,
  CommenterRunStatus
} from './commenterTypes';
import { resolve_message } from '../locales/messages';

export function run_status_label(status: CommenterRunStatus): string {
  return resolve_message(`status.${status}`);
}

export function run_progress_percent(run: CommenterRunRecord): number {
  if (run.total_jobs <= 0) {
    return 0;
  }

  const resolved = run.completed_jobs + run.review_needed_jobs + run.failed_jobs + run.skipped_jobs;
  return Math.max(0, Math.min(100, Math.round((resolved / run.total_jobs) * 100)));
}

export function run_issue_summary(run: CommenterRunRecord): string {
  const issues: string[] = [];
  if (run.review_needed_jobs > 0) {
    issues.push(resolve_message('commenter.issue.review', { count: run.review_needed_jobs }));
  }
  if (run.failed_jobs > 0) {
    issues.push(resolve_message('commenter.issue.failed', { count: run.failed_jobs }));
  }
  if (run.skipped_jobs > 0) {
    issues.push(resolve_message('commenter.issue.skipped', { count: run.skipped_jobs }));
  }

  return issues.length > 0
    ? issues.join(resolve_message('common.listSeparator'))
    : resolve_message('commenter.issue.none');
}

export function review_job_message(job: CommenterJobRecord): string {
  if (!job.error_message) {
    return resolve_message('commenter.review.default');
  }

  if (job.error_message.includes('Credential rejected')) {
    return resolve_message('commenter.review.credentials');
  }
  if (job.error_message.includes('Rejected during review')) {
    return resolve_message('commenter.review.rejected');
  }
  if (job.error_message.includes('review') || job.error_message.includes('markdown fence')) {
    return resolve_message('commenter.review.pending');
  }

  return job.error_message;
}

export function event_message(event: CommenterEventPayload): string {
  const key = `commenter.event.${event.kind}`;
  const resolved = resolve_message(key);
  return resolved === key ? event.message : resolved;
}

export function is_run_finished(status: CommenterRunStatus): boolean {
  return [
    'completed',
    'completed_with_issues',
    'cancelled',
    'failed',
    'stopped_by_limit',
    'rolled_back',
    'rollback_failed'
  ].includes(status);
}
