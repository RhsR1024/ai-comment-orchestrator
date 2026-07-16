export type CommenterRunStatus =
  | 'queued'
  | 'scanning'
  | 'ready'
  | 'running'
  | 'pausing'
  | 'paused'
  | 'stopped_by_limit'
  | 'completed'
  | 'completed_with_issues'
  | 'cancelled'
  | 'failed'
  | 'rollback_ready'
  | 'rolled_back'
  | 'rollback_failed';

export type CommenterJobStatus =
  | 'pending'
  | 'leased'
  | 'requesting'
  | 'validating'
  | 'writing'
  | 'done'
  | 'review_needed'
  | 'retry_waiting'
  | 'failed'
  | 'skipped'
  | 'rolled_back';

export type CommenterRunMode = 'auto' | 'review';
export type CommenterJsonHandlingStrategy = 'sidecar_only';
export type CommenterGlobalSettingsSection =
  | 'api-credentials'
  | 'concurrency-quota'
  | 'diff-tool'
  | 'storage-logs'
  | 'about-settings';
export type CommenterEventLevel = 'info' | 'warn' | 'error';
export type CommenterEventKind =
  | 'run_queued'
  | 'run_started'
  | 'run_paused'
  | 'run_resumed'
  | 'run_cancelled'
  | 'run_completed'
  | 'job_updated'
  | 'job_failed'
  | 'request_started'
  | 'stream_chunk'
  | 'model_response_completed'
  | 'review_requested'
  | 'review_accepted'
  | 'review_rejected'
  | 'run_rolled_back'
  | 'external_diff_opened';

export interface CommenterProjectSettings {
  default_run_mode: CommenterRunMode;
  default_max_workers: number;
  default_max_retries: number;
  default_max_files: number;
  allow_light_rewrite: boolean;
  json_handling_strategy: CommenterJsonHandlingStrategy;
  api_base_url: string;
  api_model: string;
  request_timeout_secs: number;
}

export interface CommenterProjectProfileDraft {
  project_key: string;
  profile_name: string;
  root_path: string;
  include_extensions: string[];
  exclude_directories: string[];
  prompt_template: string;
  settings: CommenterProjectSettings;
}

export interface CommenterProjectProfileView extends CommenterProjectProfileDraft {
  id: number;
  created_at: number;
  updated_at: number;
}

export interface CommenterEnqueueRunRequest {
  profile_key: string;
  requested_by: string | null;
  run_mode: CommenterRunMode;
  max_workers: number;
  max_retries: number;
  max_files: number;
  allow_light_rewrite: boolean;
  json_handling_strategy: CommenterJsonHandlingStrategy;
}

export interface CommenterJobRecord {
  id: number;
  relative_path: string;
  status: CommenterJobStatus;
  language_hint: string | null;
  write_strategy: string;
  retry_count: number;
  error_message: string | null;
  before_artifact_path: string | null;
  candidate_artifact_path: string | null;
  sidecar_artifact_path: string | null;
}

export interface CommenterRunRecord {
  run_key: string;
  profile_key: string;
  status: CommenterRunStatus;
  requested_by: string | null;
  run_mode: CommenterRunMode;
  total_jobs: number;
  completed_jobs: number;
  review_needed_jobs: number;
  failed_jobs: number;
  skipped_jobs: number;
  pending_jobs: number;
  current_file: string | null;
  max_workers: number;
  max_retries: number;
  max_files: number;
  allow_light_rewrite: boolean;
  json_handling_strategy: CommenterJsonHandlingStrategy;
  created_at: number;
  updated_at: number;
  started_at: number | null;
  finished_at: number | null;
}

export type CommenterRunHandle = CommenterRunRecord;

export interface CommenterEventPayload {
  kind: CommenterEventKind;
  run_key: string;
  relative_path: string | null;
  level: CommenterEventLevel;
  message: string;
  created_at: number;
}

export interface CommenterRunDetail {
  run: CommenterRunRecord;
  jobs: CommenterJobRecord[];
  events: CommenterEventPayload[];
}

export interface CommenterReviewActionRequest {
  run_key: string;
  relative_path: string;
}

export interface CommenterRollbackSummary {
  run_key: string;
  rolled_back_files: string[];
  conflicted_files: string[];
}

export interface CommenterDiffToolSettings {
  command_template: string;
}

export interface CommenterRunSettingsView {
  global_max_workers: number;
  api_concurrency_limit: number;
  api_bearer_token: string;
}

export interface CommenterDataPaths {
  data_root: string;
  artifacts_root: string;
  database_path: string;
  state_snapshot_path: string;
}

export type CommenterDirEntryKind = 'dir' | 'file';

export interface CommenterDirEntry {
  name: string;
  kind: CommenterDirEntryKind;
  relative_path: string;
}
