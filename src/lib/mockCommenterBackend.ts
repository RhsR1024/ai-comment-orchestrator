import type {
  CommenterDiffToolSettings,
  CommenterDirEntry,
  CommenterEnqueueRunRequest,
  CommenterEventPayload,
  CommenterJobRecord,
  CommenterProjectProfileDraft,
  CommenterProjectProfileView,
  CommenterReviewActionRequest,
  CommenterRollbackSummary,
  CommenterRunDetail,
  CommenterRunHandle,
  CommenterRunRecord,
  CommenterRunSettingsView
} from './commenterTypes';

interface MockDatabase {
  counters: {
    profile_id: number;
    run_id: number;
    job_id: number;
  };
  app_settings: CommenterRunSettingsView;
  diff_tool_settings: CommenterDiffToolSettings;
  profiles: CommenterProjectProfileView[];
  runs: Record<string, CommenterRunDetail>;
}

const STORAGE_KEY = 'ai-comment-orchestrator.commenter.mock.v1';

let memoryDatabase: MockDatabase | null = null;

function defaultDatabase(): MockDatabase {
  return {
    counters: {
      profile_id: 1,
      run_id: 1,
      job_id: 1
    },
    app_settings: {
      global_max_workers: 2,
      api_concurrency_limit: 2,
      api_bearer_token: ''
    },
    diff_tool_settings: {
      command_template: 'code --diff "{before}" "{after}"'
    },
    profiles: [],
    runs: {}
  };
}

function hasLocalStorage(): boolean {
  return typeof window !== 'undefined' && typeof window.localStorage !== 'undefined';
}

function loadDatabase(): MockDatabase {
  if (!hasLocalStorage()) {
    memoryDatabase ??= defaultDatabase();
    return memoryDatabase;
  }

  const raw = window.localStorage.getItem(STORAGE_KEY);
  if (!raw) {
    const database = defaultDatabase();
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(database));
    return database;
  }

  const database = JSON.parse(raw) as MockDatabase;
  database.app_settings.api_bearer_token ??= '';
  return database;
}

function saveDatabase(database: MockDatabase) {
  if (hasLocalStorage()) {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(database));
  } else {
    memoryDatabase = database;
  }
}

function now(): number {
  return Date.now();
}

function cloneDetail(detail: CommenterRunDetail): CommenterRunDetail {
  return JSON.parse(JSON.stringify(detail)) as CommenterRunDetail;
}

function nextProfileId(database: MockDatabase): number {
  const value = database.counters.profile_id;
  database.counters.profile_id += 1;
  return value;
}

function nextRunKey(database: MockDatabase): string {
  const value = database.counters.run_id;
  database.counters.run_id += 1;
  return `run-${String(value).padStart(4, '0')}`;
}

function nextJobId(database: MockDatabase): number {
  const value = database.counters.job_id;
  database.counters.job_id += 1;
  return value;
}

function buildSyntheticJobs(
  database: MockDatabase,
  profile: CommenterProjectProfileView
): CommenterJobRecord[] {
  const extensions = profile.include_extensions.length > 0 ? profile.include_extensions : ['go', 'ts', 'json'];
  const jobs: CommenterJobRecord[] = [];
  const basePaths = ['src/main', 'src/worker', 'src/feature'];

  for (const [index, extension] of extensions.slice(0, 3).entries()) {
    const normalized = extension.replace(/^\./, '');
    const relativePath =
      normalized === 'json'
        ? 'config/runtime.json'
        : `${basePaths[index] ?? `src/file-${index + 1}`}.${normalized}`;
    jobs.push({
      id: nextJobId(database),
      relative_path: relativePath,
      status: 'pending',
      language_hint: normalized,
      write_strategy: normalized === 'json' ? 'sidecar_only' : 'annotate_in_place',
      retry_count: 0,
      error_message: null,
      before_artifact_path: null,
      candidate_artifact_path: null,
      sidecar_artifact_path: null
    });
  }

  if (!jobs.some((job) => job.relative_path.endsWith('.json'))) {
    jobs.push({
      id: nextJobId(database),
      relative_path: 'config/runtime.json',
      status: 'pending',
      language_hint: 'json',
      write_strategy: 'sidecar_only',
      retry_count: 0,
      error_message: null,
      before_artifact_path: null,
      candidate_artifact_path: null,
      sidecar_artifact_path: null
    });
  }

  return jobs;
}

function createEvent(
  kind: CommenterEventPayload['kind'],
  runKey: string,
  message: string,
  relativePath: string | null,
  level: CommenterEventPayload['level'] = 'info'
): CommenterEventPayload {
  return {
    kind,
    run_key: runKey,
    relative_path: relativePath,
    level,
    message,
    created_at: now()
  };
}

function recalculateRun(run: CommenterRunRecord, jobs: CommenterJobRecord[]) {
  run.total_jobs = jobs.length;
  run.completed_jobs = jobs.filter((job) => job.status === 'done' || job.status === 'rolled_back').length;
  run.review_needed_jobs = jobs.filter((job) => job.status === 'review_needed').length;
  run.failed_jobs = jobs.filter((job) => job.status === 'failed').length;
  run.skipped_jobs = jobs.filter((job) => job.status === 'skipped').length;
  run.pending_jobs = jobs.filter((job) =>
    ['pending', 'leased', 'requesting', 'validating', 'writing', 'retry_waiting'].includes(job.status)
  ).length;
  run.updated_at = now();
}

function deriveRunTerminalStatus(run: CommenterRunRecord): CommenterRunRecord['status'] {
  if (run.review_needed_jobs > 0 || run.failed_jobs > 0) {
    return 'completed_with_issues';
  }
  return 'completed';
}

function isMissingCredential(database: MockDatabase): boolean {
  return database.app_settings.api_bearer_token.trim().length === 0;
}

function isBadCredential(database: MockDatabase): boolean {
  return database.app_settings.api_bearer_token.toLowerCase().includes('bad');
}

function processRunDetail(database: MockDatabase, detail: CommenterRunDetail) {
  const profile = database.profiles.find((entry) => entry.project_key === detail.run.profile_key);
  if (!profile) {
    throw new Error(`Unknown profile ${detail.run.profile_key}`);
  }

  const maxFiles = detail.run.max_files > 0 ? detail.run.max_files : Number.MAX_SAFE_INTEGER;
  let processed = 0;

  detail.run.status = 'running';
  detail.run.started_at ??= now();
  detail.events.unshift(createEvent('run_started', detail.run.run_key, 'Run started', null));

  for (const job of detail.jobs) {
    if (!['pending', 'retry_waiting'].includes(job.status)) {
      continue;
    }
    if (processed >= maxFiles) {
      detail.run.status = 'stopped_by_limit';
      break;
    }

    detail.run.current_file = job.relative_path;
    job.status = 'requesting';

    if (isMissingCredential(database) || isBadCredential(database)) {
      if (job.retry_count < detail.run.max_retries) {
        job.retry_count += 1;
        job.status = 'retry_waiting';
        job.error_message = 'Credential rejected, retry scheduled.';
      } else {
        job.status = 'failed';
        job.error_message = 'Credential rejected after retry budget was exhausted.';
      }
      detail.events.unshift(
        createEvent('job_updated', detail.run.run_key, job.error_message ?? 'Credential issue', job.relative_path, 'warn')
      );
      processed += 1;
      continue;
    }

    if (job.write_strategy !== 'sidecar_only') {
      detail.events.unshift(
        createEvent('request_started', detail.run.run_key, `AI request started for ${job.relative_path}`, job.relative_path)
      );
      detail.events.unshift(
        createEvent('stream_chunk', detail.run.run_key, `// mock stream chunk for ${job.relative_path}`, job.relative_path)
      );
      detail.events.unshift(
        createEvent('model_response_completed', detail.run.run_key, 'AI response completed: mock payload', job.relative_path)
      );
    }

    if (job.write_strategy === 'sidecar_only') {
      job.status = 'done';
      job.sidecar_artifact_path = `mock/runs/${detail.run.run_key}/sidecars/${job.relative_path}.commentary.txt`;
      detail.events.unshift(
        createEvent('job_updated', detail.run.run_key, 'Sidecar generated', job.relative_path)
      );
      processed += 1;
      continue;
    }

    job.before_artifact_path = `mock/runs/${detail.run.run_key}/before/${job.relative_path}.before`;
    job.candidate_artifact_path = `mock/runs/${detail.run.run_key}/candidates/${job.relative_path}.candidate`;

    if (detail.run.run_mode === 'review') {
      job.status = 'review_needed';
      detail.events.unshift(
        createEvent('review_requested', detail.run.run_key, 'Candidate staged for review', job.relative_path)
      );
    } else {
      job.status = 'done';
      detail.events.unshift(
        createEvent('job_updated', detail.run.run_key, 'Candidate written to source', job.relative_path)
      );
    }

    processed += 1;
  }

  detail.run.current_file = null;
  recalculateRun(detail.run, detail.jobs);
  if (detail.run.status !== 'stopped_by_limit') {
    detail.run.status = deriveRunTerminalStatus(detail.run);
  }
  detail.run.finished_at = now();
}

function updateRun(database: MockDatabase, detail: CommenterRunDetail) {
  database.runs[detail.run.run_key] = cloneDetail(detail);
}

export const mockCommenterBackend = {
  async listProjectProfiles(): Promise<CommenterProjectProfileView[]> {
    const database = loadDatabase();
    return [...database.profiles].sort((left, right) => right.updated_at - left.updated_at);
  },

  async upsertProjectProfile(request: CommenterProjectProfileDraft): Promise<CommenterProjectProfileView> {
    const database = loadDatabase();
    const timestamp = now();
    const existing = database.profiles.find((profile) => profile.project_key === request.project_key);
    const profile: CommenterProjectProfileView = {
      ...request,
      id: existing?.id ?? nextProfileId(database),
      created_at: existing?.created_at ?? timestamp,
      updated_at: timestamp
    };

    database.profiles = database.profiles.filter((entry) => entry.project_key !== request.project_key);
    database.profiles.unshift(profile);
    saveDatabase(database);
    return profile;
  },

  async enqueueRun(request: CommenterEnqueueRunRequest): Promise<CommenterRunHandle> {
    const database = loadDatabase();
    const profile = database.profiles.find((entry) => entry.project_key === request.profile_key);
    if (!profile) {
      throw new Error(`Unknown profile ${request.profile_key}`);
    }

    const run_key = nextRunKey(database);
    const timestamp = now();
    const detail: CommenterRunDetail = {
      run: {
        run_key,
        profile_key: request.profile_key,
        status: 'queued',
        requested_by: request.requested_by,
        run_mode: request.run_mode,
        total_jobs: 0,
        completed_jobs: 0,
        review_needed_jobs: 0,
        failed_jobs: 0,
        skipped_jobs: 0,
        pending_jobs: 0,
        current_file: null,
        max_workers: request.max_workers,
        max_retries: request.max_retries,
        max_files: request.max_files,
        allow_light_rewrite: request.allow_light_rewrite,
        json_handling_strategy: request.json_handling_strategy,
        created_at: timestamp,
        updated_at: timestamp,
        started_at: null,
        finished_at: null
      },
      jobs: buildSyntheticJobs(database, profile),
      events: [createEvent('run_queued', run_key, `Run queued for ${profile.profile_name}`, null)]
    };

    recalculateRun(detail.run, detail.jobs);
    updateRun(database, detail);
    saveDatabase(database);
    return detail.run;
  },

  async listRuns(): Promise<CommenterRunRecord[]> {
    const database = loadDatabase();
    return Object.values(database.runs)
      .map((detail) => detail.run)
      .sort((left, right) => right.created_at - left.created_at);
  },

  async getRunDetail(run_key: string): Promise<CommenterRunDetail | null> {
    const database = loadDatabase();
    return database.runs[run_key] ? cloneDetail(database.runs[run_key]) : null;
  },

  async deleteRun(run_key: string): Promise<CommenterRunRecord> {
    const database = loadDatabase();
    const detail = database.runs[run_key];
    if (!detail) {
      throw new Error(`Unknown run ${run_key}`);
    }
    if (detail.run.status === 'running' || detail.run.status === 'pausing') {
      throw new Error('Cannot delete an active run. Cancel or pause it first.');
    }
    delete database.runs[run_key];
    saveDatabase(database);
    return { ...detail.run };
  },

  async startRun(run_key: string): Promise<CommenterRunDetail> {
    const database = loadDatabase();
    const detail = database.runs[run_key];
    if (!detail) {
      throw new Error(`Unknown run ${run_key}`);
    }
    processRunDetail(database, detail);
    updateRun(database, detail);
    saveDatabase(database);
    return cloneDetail(detail);
  },

  async pauseRun(run_key: string): Promise<CommenterRunDetail> {
    const database = loadDatabase();
    const detail = database.runs[run_key];
    if (!detail) {
      throw new Error(`Unknown run ${run_key}`);
    }
    detail.run.status = 'paused';
    detail.events.unshift(createEvent('job_updated', run_key, 'Run paused', null));
    updateRun(database, detail);
    saveDatabase(database);
    return cloneDetail(detail);
  },

  async resumeRun(run_key: string): Promise<CommenterRunDetail> {
    const database = loadDatabase();
    const detail = database.runs[run_key];
    if (!detail) {
      throw new Error(`Unknown run ${run_key}`);
    }
    processRunDetail(database, detail);
    updateRun(database, detail);
    saveDatabase(database);
    return cloneDetail(detail);
  },

  async cancelRun(run_key: string): Promise<CommenterRunDetail> {
    const database = loadDatabase();
    const detail = database.runs[run_key];
    if (!detail) {
      throw new Error(`Unknown run ${run_key}`);
    }
    detail.run.status = 'cancelled';
    detail.run.finished_at = now();
    detail.events.unshift(createEvent('job_updated', run_key, 'Run cancelled', null, 'warn'));
    updateRun(database, detail);
    saveDatabase(database);
    return cloneDetail(detail);
  },

  async listReviewJobs(): Promise<CommenterJobRecord[]> {
    const database = loadDatabase();
    return Object.values(database.runs)
      .flatMap((detail) => detail.jobs.filter((job) => job.status === 'review_needed'))
      .sort((left, right) => right.id - left.id);
  },

  async acceptReviewJob(request: CommenterReviewActionRequest): Promise<CommenterRunDetail> {
    const database = loadDatabase();
    const detail = database.runs[request.run_key];
    if (!detail) {
      throw new Error(`Unknown run ${request.run_key}`);
    }
    const job = detail.jobs.find((entry) => entry.relative_path === request.relative_path);
    if (!job) {
      throw new Error(`Unknown job ${request.relative_path}`);
    }
    job.status = 'done';
    job.error_message = null;
    detail.events.unshift(createEvent('review_accepted', request.run_key, 'Review accepted', job.relative_path));
    recalculateRun(detail.run, detail.jobs);
    detail.run.status = deriveRunTerminalStatus(detail.run);
    updateRun(database, detail);
    saveDatabase(database);
    return cloneDetail(detail);
  },

  async rejectReviewJob(request: CommenterReviewActionRequest): Promise<CommenterRunDetail> {
    const database = loadDatabase();
    const detail = database.runs[request.run_key];
    if (!detail) {
      throw new Error(`Unknown run ${request.run_key}`);
    }
    const job = detail.jobs.find((entry) => entry.relative_path === request.relative_path);
    if (!job) {
      throw new Error(`Unknown job ${request.relative_path}`);
    }
    job.status = 'skipped';
    job.error_message = 'Rejected during review.';
    detail.events.unshift(createEvent('review_rejected', request.run_key, 'Review rejected', job.relative_path, 'warn'));
    recalculateRun(detail.run, detail.jobs);
    detail.run.status = deriveRunTerminalStatus(detail.run);
    updateRun(database, detail);
    saveDatabase(database);
    return cloneDetail(detail);
  },

  async retryJob(request: CommenterReviewActionRequest): Promise<CommenterRunDetail> {
    const database = loadDatabase();
    const detail = database.runs[request.run_key];
    if (!detail) {
      throw new Error(`Unknown run ${request.run_key}`);
    }
    const job = detail.jobs.find((entry) => entry.relative_path === request.relative_path);
    if (!job) {
      throw new Error(`Unknown job ${request.relative_path}`);
    }
    job.status = 'pending';
    job.error_message = null;
    detail.events.unshift(createEvent('job_updated', request.run_key, 'Job retried', job.relative_path));
    processRunDetail(database, detail);
    updateRun(database, detail);
    saveDatabase(database);
    return cloneDetail(detail);
  },

  async openExternalDiff(request: CommenterReviewActionRequest): Promise<void> {
    const database = loadDatabase();
    const detail = database.runs[request.run_key];
    if (!detail) {
      throw new Error(`Unknown run ${request.run_key}`);
    }
    detail.events.unshift(createEvent('external_diff_opened', request.run_key, 'External diff invoked', request.relative_path));
    updateRun(database, detail);
    saveDatabase(database);
  },

  async rollbackRun(run_key: string): Promise<CommenterRollbackSummary> {
    const database = loadDatabase();
    const detail = database.runs[run_key];
    if (!detail) {
      throw new Error(`Unknown run ${run_key}`);
    }

    const rolled_back_files: string[] = [];
    const conflicted_files: string[] = [];

    for (const [index, job] of detail.jobs.entries()) {
      if (job.status !== 'done') {
        continue;
      }
      if (index === 1) {
        conflicted_files.push(job.relative_path);
      } else {
        job.status = 'rolled_back';
        rolled_back_files.push(job.relative_path);
      }
    }

    detail.run.status = conflicted_files.length > 0 ? 'rollback_failed' : 'rolled_back';
    detail.events.unshift(
      createEvent(
        'run_rolled_back',
        run_key,
        `Rollback finished with ${rolled_back_files.length} restored and ${conflicted_files.length} conflicts.`,
        null,
        conflicted_files.length > 0 ? 'warn' : 'info'
      )
    );
    recalculateRun(detail.run, detail.jobs);
    updateRun(database, detail);
    saveDatabase(database);

    return {
      run_key,
      rolled_back_files,
      conflicted_files
    };
  },

  async getAppSettings(): Promise<CommenterRunSettingsView> {
    const database = loadDatabase();
    return { ...database.app_settings };
  },

  async updateAppSettings(request: CommenterRunSettingsView): Promise<CommenterRunSettingsView> {
    const database = loadDatabase();
    database.app_settings = {
      global_max_workers: Math.max(1, request.global_max_workers),
      api_concurrency_limit: Math.max(1, request.api_concurrency_limit),
      api_bearer_token: request.api_bearer_token
    };
    saveDatabase(database);
    return { ...database.app_settings };
  },

  async getDiffToolSettings(): Promise<CommenterDiffToolSettings> {
    const database = loadDatabase();
    return { ...database.diff_tool_settings };
  },

  async updateDiffToolSettings(
    request: CommenterDiffToolSettings
  ): Promise<CommenterDiffToolSettings> {
    const database = loadDatabase();
    database.diff_tool_settings = { ...request };
    saveDatabase(database);
    return { ...database.diff_tool_settings };
  },

  listDir: async (_profile_key: string, _relative_path: string): Promise<CommenterDirEntry[]> => {
    void _profile_key;
    void _relative_path;
    return [];
  },
  getCandidateText: async (_run_key: string, _relative_path: string): Promise<string> => {
    void _run_key;
    void _relative_path;
    return '';
  },
  getDataPaths: async () => ({
    data_root: '',
    artifacts_root: '',
    database_path: '',
    state_snapshot_path: ''
  }),
  openPath: async (_path: string): Promise<void> => {
    void _path;
  }
};
