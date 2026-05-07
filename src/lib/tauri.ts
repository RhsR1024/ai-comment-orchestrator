import type {
  CommenterDiffToolSettings,
  CommenterDirEntry,
  CommenterEnqueueRunRequest,
  CommenterEventPayload,
  CommenterProjectProfileDraft,
  CommenterProjectProfileView,
  CommenterReviewActionRequest,
  CommenterRollbackSummary,
  CommenterRunDetail,
  CommenterRunHandle,
  CommenterRunRecord,
  CommenterRunSettingsView,
  CommenterJobRecord
} from './commenterTypes';
import { mockCommenterBackend } from './mockCommenterBackend';

export const COMMENTER_EVENT_CHANNEL = 'commenter://state';

function hasTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

async function tauriInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(command, args);
}

export async function pickProjectRootPath(currentPath = ''): Promise<string | null> {
  if (hasTauriRuntime()) {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const selectedPath = await open({
      directory: true,
      multiple: false,
      defaultPath: currentPath || undefined,
      title: 'Select project root path'
    });

    return typeof selectedPath === 'string' ? selectedPath : null;
  }

  if (typeof window === 'undefined') {
    return null;
  }

  const value = window.prompt('Project root path', currentPath);
  return value && value.trim().length > 0 ? value.trim() : null;
}

export async function subscribeCommenterEvents(
  handler: (payload: CommenterEventPayload) => void
): Promise<() => void> {
  if (!hasTauriRuntime()) {
    return () => undefined;
  }

  const { listen } = await import('@tauri-apps/api/event');
  const unlisten = await listen<CommenterEventPayload>(COMMENTER_EVENT_CHANNEL, (event) => {
    handler(event.payload);
  });
  return unlisten;
}

export const commenterApi = {
  listProjectProfiles: async (): Promise<CommenterProjectProfileView[]> => {
    if (hasTauriRuntime()) {
      return tauriInvoke<CommenterProjectProfileView[]>('commenter_list_project_profiles');
    }
    return mockCommenterBackend.listProjectProfiles();
  },
  upsertProjectProfile: async (
    request: CommenterProjectProfileDraft
  ): Promise<CommenterProjectProfileView> => {
    if (hasTauriRuntime()) {
      return tauriInvoke<CommenterProjectProfileView>('commenter_upsert_project_profile', { request });
    }
    return mockCommenterBackend.upsertProjectProfile(request);
  },
  enqueueRun: async (request: CommenterEnqueueRunRequest): Promise<CommenterRunHandle> => {
    if (hasTauriRuntime()) {
      return tauriInvoke<CommenterRunHandle>('commenter_enqueue_run', { request });
    }
    return mockCommenterBackend.enqueueRun(request);
  },
  listRuns: async (): Promise<CommenterRunRecord[]> => {
    if (hasTauriRuntime()) {
      return tauriInvoke<CommenterRunRecord[]>('commenter_list_runs');
    }
    return mockCommenterBackend.listRuns();
  },
  getRunDetail: async (run_key: string): Promise<CommenterRunDetail | null> => {
    if (hasTauriRuntime()) {
      return tauriInvoke<CommenterRunDetail | null>('commenter_get_run_detail', { runKey: run_key });
    }
    return mockCommenterBackend.getRunDetail(run_key);
  },
  deleteRun: async (run_key: string): Promise<CommenterRunRecord> => {
    if (hasTauriRuntime()) {
      return tauriInvoke<CommenterRunRecord>('commenter_delete_run', { runId: run_key });
    }
    return mockCommenterBackend.deleteRun(run_key);
  },
  startRun: async (run_key: string): Promise<CommenterRunDetail> => {
    if (hasTauriRuntime()) {
      return tauriInvoke<CommenterRunDetail>('commenter_start_run', { runId: run_key });
    }
    return mockCommenterBackend.startRun(run_key);
  },
  pauseRun: async (run_key: string): Promise<CommenterRunDetail> => {
    if (hasTauriRuntime()) {
      return tauriInvoke<CommenterRunDetail>('commenter_pause_run', { runId: run_key });
    }
    return mockCommenterBackend.pauseRun(run_key);
  },
  resumeRun: async (run_key: string): Promise<CommenterRunDetail> => {
    if (hasTauriRuntime()) {
      return tauriInvoke<CommenterRunDetail>('commenter_resume_run', { runId: run_key });
    }
    return mockCommenterBackend.resumeRun(run_key);
  },
  cancelRun: async (run_key: string): Promise<CommenterRunDetail> => {
    if (hasTauriRuntime()) {
      return tauriInvoke<CommenterRunDetail>('commenter_cancel_run', { runId: run_key });
    }
    return mockCommenterBackend.cancelRun(run_key);
  },
  listReviewJobs: async (): Promise<CommenterJobRecord[]> => {
    if (hasTauriRuntime()) {
      return tauriInvoke<CommenterJobRecord[]>('commenter_list_review_jobs');
    }
    return mockCommenterBackend.listReviewJobs();
  },
  acceptReviewJob: async (request: CommenterReviewActionRequest): Promise<CommenterRunDetail> => {
    if (hasTauriRuntime()) {
      return tauriInvoke<CommenterRunDetail>('commenter_accept_review_job', { request });
    }
    return mockCommenterBackend.acceptReviewJob(request);
  },
  rejectReviewJob: async (request: CommenterReviewActionRequest): Promise<CommenterRunDetail> => {
    if (hasTauriRuntime()) {
      return tauriInvoke<CommenterRunDetail>('commenter_reject_review_job', { request });
    }
    return mockCommenterBackend.rejectReviewJob(request);
  },
  retryJob: async (request: CommenterReviewActionRequest): Promise<CommenterRunDetail> => {
    if (hasTauriRuntime()) {
      return tauriInvoke<CommenterRunDetail>('commenter_retry_job', { request });
    }
    return mockCommenterBackend.retryJob(request);
  },
  openExternalDiff: async (request: CommenterReviewActionRequest): Promise<void> => {
    if (hasTauriRuntime()) {
      return tauriInvoke<void>('commenter_open_external_diff', { request });
    }
    return mockCommenterBackend.openExternalDiff(request);
  },
  rollbackRun: async (run_key: string): Promise<CommenterRollbackSummary> => {
    if (hasTauriRuntime()) {
      return tauriInvoke<CommenterRollbackSummary>('commenter_rollback_run', { runId: run_key });
    }
    return mockCommenterBackend.rollbackRun(run_key);
  },
  getAppSettings: async (): Promise<CommenterRunSettingsView> => {
    if (hasTauriRuntime()) {
      return tauriInvoke<CommenterRunSettingsView>('commenter_get_app_settings');
    }
    return mockCommenterBackend.getAppSettings();
  },
  updateAppSettings: async (
    request: CommenterRunSettingsView
  ): Promise<CommenterRunSettingsView> => {
    if (hasTauriRuntime()) {
      return tauriInvoke<CommenterRunSettingsView>('commenter_update_app_settings', { request });
    }
    return mockCommenterBackend.updateAppSettings(request);
  },
  getDiffToolSettings: async (): Promise<CommenterDiffToolSettings> => {
    if (hasTauriRuntime()) {
      return tauriInvoke<CommenterDiffToolSettings>('commenter_get_diff_tool_settings');
    }
    return mockCommenterBackend.getDiffToolSettings();
  },
  updateDiffToolSettings: async (
    request: CommenterDiffToolSettings
  ): Promise<CommenterDiffToolSettings> => {
    if (hasTauriRuntime()) {
      return tauriInvoke<CommenterDiffToolSettings>('commenter_update_diff_tool_settings', { request });
    }
    return mockCommenterBackend.updateDiffToolSettings(request);
  },
  listDir: async (profile_key: string, relative_path: string): Promise<CommenterDirEntry[]> => {
    if (hasTauriRuntime()) {
      return tauriInvoke<CommenterDirEntry[]>('commenter_list_dir', {
        profileKey: profile_key,
        relativePath: relative_path
      });
    }
    return mockCommenterBackend.listDir(profile_key, relative_path);
  },
  getCandidateText: async (run_key: string, relative_path: string): Promise<string> => {
    if (hasTauriRuntime()) {
      return tauriInvoke<string>('commenter_get_candidate_text', {
        runKey: run_key,
        relativePath: relative_path
      });
    }
    return mockCommenterBackend.getCandidateText(run_key, relative_path);
  }
};
