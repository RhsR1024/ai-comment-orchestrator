use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use futures_util::{stream::FuturesUnordered, StreamExt};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(not(test))]
use tauri::AppHandle;
#[cfg(test)]
type AppHandle = ();

use super::{
    artifacts::CommenterRunPaths,
    config::{describe_bearer_token_source, resolve_bearer_token},
    db::open_app_database,
    events::{emit_commenter_event, CommenterEventKind, CommenterEventPayload},
    http::{
        build_chat_completions_request_debug, call_chat_completions_with_debug,
        ChatCompletionsRequest, ChatCompletionsRequestContext, DEFAULT_API_BASE_URL,
        DEFAULT_API_MODEL, DEFAULT_MAX_TOKENS, DEFAULT_REQUEST_TIMEOUT_SECS,
    },
    models::{
        CommentAppSettings, CommentJobStatus, CommentProjectSettings, CommentRunMode,
        CommentRunStatus, EventLevel, JsonHandlingStrategy,
    },
    prompt::{build_annotation_prompt, AnnotationPromptParts},
    rollback::{can_overwrite_for_rollback, RollbackGuard},
    scanner::{scan_project_tree, FileKind, ScannedFile, WriteStrategy},
    scheduler::recover_run_status,
    telemetry::CodebuddyTelemetry,
    validate::{validate_candidate, FileValidationInput, ValidationDecision},
};

const COMMENTER_STATE_FILE_NAME: &str = "commenter-state.json";
const STREAM_EVENT_FLUSH_CHARS: usize = 512;
const STREAM_EVENT_FLUSH_INTERVAL: Duration = Duration::from_millis(100);
const STREAM_EVENT_MAX_CHARS: usize = 1200;

pub const COMMENTER_COMMAND_NAMES: &[&str] = &[
    "commenter_upsert_project_profile",
    "commenter_list_project_profiles",
    "commenter_delete_project_profile",
    "commenter_enqueue_run",
    "commenter_list_runs",
    "commenter_get_run_detail",
    "commenter_delete_run",
    "commenter_start_run",
    "commenter_pause_run",
    "commenter_resume_run",
    "commenter_cancel_run",
    "commenter_list_review_jobs",
    "commenter_accept_review_job",
    "commenter_reject_review_job",
    "commenter_retry_job",
    "commenter_open_external_diff",
    "commenter_rollback_run",
    "commenter_get_app_settings",
    "commenter_update_app_settings",
    "commenter_get_diff_tool_settings",
    "commenter_update_diff_tool_settings",
    "commenter_list_dir",
    "commenter_get_candidate_text",
    "commenter_get_original_text",
    "commenter_get_data_paths",
    "commenter_open_path",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommenterProjectProfileDraft {
    pub project_key: String,
    pub profile_name: String,
    pub root_path: String,
    pub include_extensions: Vec<String>,
    pub exclude_directories: Vec<String>,
    pub prompt_template: String,
    pub settings: CommentProjectSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommenterProjectProfileView {
    pub id: i64,
    pub project_key: String,
    pub profile_name: String,
    pub root_path: String,
    pub include_extensions: Vec<String>,
    pub exclude_directories: Vec<String>,
    pub prompt_template: String,
    pub settings: CommentProjectSettings,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommenterEnqueueRunRequest {
    pub profile_key: String,
    pub requested_by: Option<String>,
    pub run_mode: CommentRunMode,
    pub max_workers: i64,
    pub max_retries: i64,
    pub max_files: i64,
    pub allow_light_rewrite: bool,
    pub json_handling_strategy: JsonHandlingStrategy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommenterJobRecord {
    pub id: i64,
    pub relative_path: String,
    pub status: CommentJobStatus,
    pub language_hint: Option<String>,
    pub write_strategy: String,
    pub retry_count: i64,
    pub error_message: Option<String>,
    pub before_artifact_path: Option<String>,
    pub candidate_artifact_path: Option<String>,
    pub sidecar_artifact_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommenterRunRecord {
    pub run_key: String,
    pub profile_key: String,
    pub status: CommentRunStatus,
    pub requested_by: Option<String>,
    pub run_mode: CommentRunMode,
    pub total_jobs: i64,
    pub completed_jobs: i64,
    pub review_needed_jobs: i64,
    pub failed_jobs: i64,
    pub skipped_jobs: i64,
    pub pending_jobs: i64,
    pub current_file: Option<String>,
    pub max_workers: i64,
    pub max_retries: i64,
    pub max_files: i64,
    pub allow_light_rewrite: bool,
    pub json_handling_strategy: JsonHandlingStrategy,
    pub created_at: i64,
    pub updated_at: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
}

pub type CommenterRunHandle = CommenterRunRecord;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommenterRunDetail {
    pub run: CommenterRunRecord,
    pub jobs: Vec<CommenterJobRecord>,
    pub events: Vec<CommenterEventPayload>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommenterReviewActionRequest {
    pub run_key: String,
    pub relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommenterRollbackSummary {
    pub run_key: String,
    pub rolled_back_files: Vec<String>,
    pub conflicted_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommenterDiffToolSettings {
    pub command_template: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommenterRunSettingsView {
    pub global_max_workers: i64,
    pub api_concurrency_limit: i64,
    pub api_bearer_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommenterDataPaths {
    pub data_root: String,
    pub artifacts_root: String,
    pub database_path: String,
    pub state_snapshot_path: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommenterDirEntryKind {
    Dir,
    File,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommenterDirEntry {
    pub name: String,
    pub kind: CommenterDirEntryKind,
    pub relative_path: String,
}

#[derive(Debug)]
pub struct CommenterCommandSurface {
    state: Mutex<CommenterState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommenterState {
    data_root: PathBuf,
    next_profile_id: i64,
    next_run_id: i64,
    next_job_id: i64,
    app_settings: CommentAppSettings,
    diff_tool_settings: CommenterDiffToolSettings,
    profiles: BTreeMap<String, CommenterProjectProfileView>,
    runs: BTreeMap<String, StoredRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredRun {
    run: CommenterRunRecord,
    jobs: Vec<StoredJob>,
    events: Vec<CommenterEventPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredJob {
    record: CommenterJobRecord,
    absolute_path: PathBuf,
    kind: FileKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    candidate_content: Option<String>,
    before_hash: Option<String>,
    written_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    telemetry_context: Option<ChatCompletionsRequestContext>,
}

impl CommenterCommandSurface {
    pub fn new(data_root: impl Into<PathBuf>) -> Self {
        let data_root = data_root.into();
        let _ = open_app_database(&data_root);
        Self {
            state: Mutex::new(load_state(data_root)),
        }
    }

    pub fn data_root(&self) -> PathBuf {
        self.state
            .lock()
            .expect("state lock poisoned")
            .data_root
            .clone()
    }

    pub fn data_paths(&self) -> Result<CommenterDataPaths, String> {
        let data_root = self.data_root();
        let artifacts_root = data_root.join("commenter").join("runs");
        let database_path = data_root.join(crate::commenter::db::COMMENTER_DB_FILE_NAME);
        let state_snapshot_path = data_root.join(COMMENTER_STATE_FILE_NAME);
        Ok(CommenterDataPaths {
            data_root: data_root.to_string_lossy().into_owned(),
            artifacts_root: artifacts_root.to_string_lossy().into_owned(),
            database_path: database_path.to_string_lossy().into_owned(),
            state_snapshot_path: state_snapshot_path.to_string_lossy().into_owned(),
        })
    }

    pub fn upsert_project_profile(
        &self,
        request: CommenterProjectProfileDraft,
    ) -> Result<CommenterProjectProfileView, String> {
        let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;
        let timestamp = unix_timestamp_now();
        let existing = state.profiles.get(&request.project_key).cloned();
        let profile = CommenterProjectProfileView {
            id: existing.as_ref().map(|value| value.id).unwrap_or_else(|| {
                let id = state.next_profile_id;
                state.next_profile_id += 1;
                id
            }),
            project_key: request.project_key.clone(),
            profile_name: request.profile_name,
            root_path: request.root_path,
            include_extensions: normalize_extensions(&request.include_extensions),
            exclude_directories: request.exclude_directories,
            prompt_template: request.prompt_template,
            settings: request.settings,
            created_at: existing
                .as_ref()
                .map(|value| value.created_at)
                .unwrap_or(timestamp),
            updated_at: timestamp,
        };
        state
            .profiles
            .insert(profile.project_key.clone(), profile.clone());
        persist_locked_state(&state)?;
        Ok(profile)
    }

    pub fn list_project_profiles(&self) -> Result<Vec<CommenterProjectProfileView>, String> {
        let state = self.state.lock().map_err(|_| "state lock poisoned")?;
        Ok(state.profiles.values().cloned().collect())
    }

    pub fn delete_project_profile(
        &self,
        project_key: &str,
    ) -> Result<CommenterProjectProfileView, String> {
        let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;
        let profile = state
            .profiles
            .get(project_key)
            .cloned()
            .ok_or_else(|| format!("unknown profile: {project_key}"))?;
        if state
            .runs
            .values()
            .any(|stored_run| stored_run.run.profile_key == project_key)
        {
            return Err(
                "cannot delete a project that is referenced by a run; delete its runs first"
                    .to_string(),
            );
        }
        state.profiles.remove(project_key);
        persist_locked_state(&state)?;
        Ok(profile)
    }

    pub fn list_dir(
        &self,
        profile_key: &str,
        relative_path: &str,
    ) -> Result<Vec<CommenterDirEntry>, String> {
        let profiles = self.list_project_profiles()?;
        let profile = profiles
            .iter()
            .find(|p| p.project_key == profile_key)
            .ok_or_else(|| format!("profile not found: {profile_key}"))?;

        let root = std::path::PathBuf::from(&profile.root_path);
        let canonical_root = root
            .canonicalize()
            .map_err(|e| format!("profile root unreadable: {e}"))?;

        let target = if relative_path.is_empty() {
            canonical_root.clone()
        } else {
            canonical_root.join(relative_path)
        };
        let canonical_target = target
            .canonicalize()
            .map_err(|e| format!("path not accessible: {e}"))?;
        if !canonical_target.starts_with(&canonical_root) {
            return Err("path escapes profile root".to_string());
        }

        let exclude: std::collections::HashSet<&str> = profile
            .exclude_directories
            .iter()
            .map(|s| s.as_str())
            .collect();

        let mut entries = Vec::new();
        let read = std::fs::read_dir(&canonical_target).map_err(|e| e.to_string())?;
        for raw in read {
            let raw = raw.map_err(|e| e.to_string())?;
            let name = raw.file_name().to_string_lossy().into_owned();
            let metadata = raw.metadata().map_err(|e| e.to_string())?;
            let is_dir = metadata.is_dir();

            if is_dir && exclude.contains(name.as_str()) {
                continue;
            }

            let child_relative = if relative_path.is_empty() {
                name.clone()
            } else {
                format!("{relative_path}/{name}")
            };

            entries.push(CommenterDirEntry {
                name,
                kind: if is_dir {
                    CommenterDirEntryKind::Dir
                } else {
                    CommenterDirEntryKind::File
                },
                relative_path: child_relative,
            });
        }

        entries.sort_by(|a, b| match (&a.kind, &b.kind) {
            (CommenterDirEntryKind::Dir, CommenterDirEntryKind::File) => std::cmp::Ordering::Less,
            (CommenterDirEntryKind::File, CommenterDirEntryKind::Dir) => {
                std::cmp::Ordering::Greater
            }
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });

        Ok(entries)
    }

    pub fn get_candidate_text(&self, run_key: &str, relative_path: &str) -> Result<String, String> {
        let run_paths =
            crate::commenter::artifacts::CommenterRunPaths::new(&self.data_root(), run_key)
                .map_err(|e| e.to_string())?;
        let candidate = read_artifact_text(
            &run_paths.candidate_root,
            relative_path,
            ".candidate",
            "candidate path escapes run root",
        )?;
        if !candidate.is_empty() {
            return Ok(candidate);
        }

        read_artifact_text(
            &run_paths.sidecar_root,
            relative_path,
            ".commentary.txt",
            "sidecar path escapes run root",
        )
    }

    pub fn get_original_text(&self, run_key: &str, relative_path: &str) -> Result<String, String> {
        let run_paths =
            crate::commenter::artifacts::CommenterRunPaths::new(&self.data_root(), run_key)
                .map_err(|error| error.to_string())?;
        let before_path = artifact_output_path(&run_paths.before_root, relative_path, ".before");
        let before_exists = before_path.is_file();
        let before = read_artifact_text(
            &run_paths.before_root,
            relative_path,
            ".before",
            "before path escapes run root",
        )?;
        if before_exists {
            return Ok(before);
        }

        let source_path = {
            let state = self.state.lock().map_err(|_| "state lock poisoned")?;
            state
                .runs
                .get(run_key)
                .and_then(|run| {
                    run.jobs
                        .iter()
                        .find(|job| job.record.relative_path == relative_path)
                })
                .map(|job| job.absolute_path.clone())
        };

        match source_path {
            Some(path) => std::fs::read_to_string(path).map_err(|error| error.to_string()),
            None => Ok(String::new()),
        }
    }

    pub fn enqueue_run(
        &self,
        request: CommenterEnqueueRunRequest,
    ) -> Result<CommenterRunHandle, String> {
        let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;
        let profile = state
            .profiles
            .get(&request.profile_key)
            .cloned()
            .ok_or_else(|| format!("unknown profile: {}", request.profile_key))?;
        let run_key = format!("run-{:04}", state.next_run_id);
        state.next_run_id += 1;

        let scanned_files = scan_project_tree(
            Path::new(&profile.root_path),
            &profile.include_extensions,
            &profile.exclude_directories,
        )
        .map_err(|error| error.to_string())?;

        let jobs = scanned_files
            .into_iter()
            .map(|file| build_stored_job(&mut state, file))
            .collect::<Vec<_>>();
        let timestamp = unix_timestamp_now();
        let run = CommenterRunRecord {
            run_key: run_key.clone(),
            profile_key: profile.project_key.clone(),
            status: if jobs.is_empty() {
                CommentRunStatus::Completed
            } else {
                CommentRunStatus::Queued
            },
            requested_by: request.requested_by,
            run_mode: request.run_mode,
            total_jobs: jobs.len() as i64,
            completed_jobs: 0,
            review_needed_jobs: 0,
            failed_jobs: 0,
            skipped_jobs: 0,
            pending_jobs: jobs.len() as i64,
            current_file: None,
            max_workers: request.max_workers,
            max_retries: request.max_retries,
            max_files: request.max_files,
            allow_light_rewrite: request.allow_light_rewrite,
            json_handling_strategy: request.json_handling_strategy,
            created_at: timestamp,
            updated_at: timestamp,
            started_at: None,
            finished_at: None,
        };

        let mut stored_run = StoredRun {
            run,
            jobs,
            events: Vec::new(),
        };
        push_event(
            &mut stored_run.events,
            CommenterEventPayload {
                kind: CommenterEventKind::RunQueued,
                run_key: run_key.clone(),
                relative_path: None,
                level: EventLevel::Info,
                message: format!("Run {} queued for {}", run_key, profile.profile_name),
                created_at: timestamp,
            },
        );
        recalculate_run_counts(&mut stored_run.run, &stored_run.jobs);
        let handle = stored_run.run.clone();
        state.runs.insert(run_key, stored_run);
        persist_locked_state(&state)?;
        Ok(handle)
    }

    pub fn list_runs(&self) -> Result<Vec<CommenterRunRecord>, String> {
        let state = self.state.lock().map_err(|_| "state lock poisoned")?;
        Ok(state
            .runs
            .values()
            .map(|value| value.run.clone())
            .collect::<Vec<_>>())
    }

    pub fn get_run_detail(&self, run_key: &str) -> Result<Option<CommenterRunDetail>, String> {
        let state = self.state.lock().map_err(|_| "state lock poisoned")?;
        Ok(state.runs.get(run_key).map(stored_run_detail))
    }

    pub fn delete_run(&self, run_key: &str) -> Result<CommenterRunRecord, String> {
        let (removed_run, artifact_root) = {
            let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;
            let status = state
                .runs
                .get(run_key)
                .map(|stored_run| stored_run.run.status)
                .ok_or_else(|| format!("unknown run: {run_key}"))?;
            if matches!(
                status,
                CommentRunStatus::Running | CommentRunStatus::Pausing
            ) {
                return Err("cannot delete an active run; cancel or pause it first".to_string());
            }

            let removed = state
                .runs
                .remove(run_key)
                .ok_or_else(|| format!("unknown run: {run_key}"))?;
            persist_locked_state(&state)?;
            let run_paths = CommenterRunPaths::new(&state.data_root, run_key)
                .map_err(|error| error.to_string())?;
            (removed.run, run_paths.run_root)
        };

        let _ = fs::remove_dir_all(artifact_root);

        Ok(removed_run)
    }

    pub async fn start_run(
        &self,
        app: Option<&AppHandle>,
        run_key: &str,
    ) -> Result<CommenterRunDetail, String> {
        self.process_run(app, run_key, false).await
    }

    pub fn pause_run(&self, run_key: &str) -> Result<CommenterRunDetail, String> {
        let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;
        let stored_run = state
            .runs
            .get_mut(run_key)
            .ok_or_else(|| format!("unknown run: {run_key}"))?;
        // Running -> Pausing 让正在跑的循环看到信号后自然落到 Paused；
        // 否则（已停止 / 已完成 / 还没开始）直接置 Paused。
        stored_run.run.status = if stored_run.run.status == CommentRunStatus::Running {
            CommentRunStatus::Pausing
        } else {
            CommentRunStatus::Paused
        };
        stored_run.run.updated_at = unix_timestamp_now();
        push_event(
            &mut stored_run.events,
            CommenterEventPayload {
                kind: CommenterEventKind::RunPaused,
                run_key: run_key.to_string(),
                relative_path: stored_run.run.current_file.clone(),
                level: EventLevel::Info,
                message: "Run paused".to_string(),
                created_at: stored_run.run.updated_at,
            },
        );
        let detail = stored_run_detail(stored_run);
        persist_locked_state(&state)?;
        Ok(detail)
    }

    pub async fn resume_run(
        &self,
        app: Option<&AppHandle>,
        run_key: &str,
    ) -> Result<CommenterRunDetail, String> {
        self.process_run(app, run_key, true).await
    }

    pub fn cancel_run(&self, run_key: &str) -> Result<CommenterRunDetail, String> {
        let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;
        let stored_run = state
            .runs
            .get_mut(run_key)
            .ok_or_else(|| format!("unknown run: {run_key}"))?;
        stored_run.run.status = CommentRunStatus::Cancelled;
        stored_run.run.current_file = None;
        stored_run.run.updated_at = unix_timestamp_now();
        stored_run.run.finished_at = Some(stored_run.run.updated_at);
        push_event(
            &mut stored_run.events,
            CommenterEventPayload {
                kind: CommenterEventKind::RunCancelled,
                run_key: run_key.to_string(),
                relative_path: None,
                level: EventLevel::Warn,
                message: "Run cancelled".to_string(),
                created_at: stored_run.run.updated_at,
            },
        );
        let detail = stored_run_detail(stored_run);
        persist_locked_state(&state)?;
        Ok(detail)
    }

    pub fn list_review_jobs(&self) -> Result<Vec<CommenterJobRecord>, String> {
        let state = self.state.lock().map_err(|_| "state lock poisoned")?;
        let mut jobs = Vec::new();
        for run in state.runs.values() {
            jobs.extend(
                run.jobs
                    .iter()
                    .filter(|job| job.record.status == CommentJobStatus::ReviewNeeded)
                    .map(|job| job.record.clone()),
            );
        }
        Ok(jobs)
    }

    pub fn accept_review_job(
        &self,
        request: CommenterReviewActionRequest,
    ) -> Result<CommenterRunDetail, String> {
        let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;
        let data_root = state.data_root.clone();
        let write_telemetry = state
            .runs
            .get(&request.run_key)
            .and_then(|run| state.profiles.get(&run.run.profile_key))
            .and_then(|profile| build_write_telemetry(profile, &state.app_settings).ok());
        let stored_run = state
            .runs
            .get_mut(&request.run_key)
            .ok_or_else(|| format!("unknown run: {}", request.run_key))?;
        let run_paths = CommenterRunPaths::new(&data_root, &request.run_key)
            .map_err(|error| error.to_string())?;
        let timestamp = unix_timestamp_now();

        let Some(job) = stored_run
            .jobs
            .iter_mut()
            .find(|job| job.record.relative_path == request.relative_path)
        else {
            return Err(format!("unknown job: {}", request.relative_path));
        };

        let candidate_path = artifact_output_path(
            &run_paths.candidate_root,
            &job.record.relative_path,
            ".candidate",
        );
        let candidate_content = fs::read_to_string(&candidate_path)
            .or_else(|artifact_error| job.candidate_content.clone().ok_or(artifact_error))
            .map_err(|error| format!("read candidate artifact failed: {error}"))?;
        let before_content = fs::read_to_string(&job.absolute_path).unwrap_or_default();
        let relative_path = job.record.relative_path.clone();
        write_before_snapshot(&run_paths, &relative_path, &before_content, job)?;
        fs::write(&job.absolute_path, &candidate_content).map_err(|error| error.to_string())?;
        if let Some((telemetry, context)) = &write_telemetry {
            let context = job.telemetry_context.as_ref().unwrap_or(context);
            telemetry.report_file_write(
                context,
                &relative_path,
                &before_content,
                &candidate_content,
            );
        }
        job.record.status = CommentJobStatus::Done;
        job.record.error_message = None;
        job.written_hash = Some(hash_content(&candidate_content));
        job.record.candidate_artifact_path = Some(candidate_path.to_string_lossy().to_string());
        job.candidate_content = None;
        job.telemetry_context = None;
        stored_run.run.updated_at = timestamp;
        push_event(
            &mut stored_run.events,
            CommenterEventPayload {
                kind: CommenterEventKind::ReviewAccepted,
                run_key: request.run_key,
                relative_path: Some(job.record.relative_path.clone()),
                level: EventLevel::Info,
                message: "Review job accepted and written".to_string(),
                created_at: timestamp,
            },
        );
        recalculate_run_counts(&mut stored_run.run, &stored_run.jobs);
        let detail = stored_run_detail(stored_run);
        persist_locked_state(&state)?;
        Ok(detail)
    }

    pub fn reject_review_job(
        &self,
        request: CommenterReviewActionRequest,
    ) -> Result<CommenterRunDetail, String> {
        let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;
        let stored_run = state
            .runs
            .get_mut(&request.run_key)
            .ok_or_else(|| format!("unknown run: {}", request.run_key))?;
        let Some(job) = stored_run
            .jobs
            .iter_mut()
            .find(|job| job.record.relative_path == request.relative_path)
        else {
            return Err(format!("unknown job: {}", request.relative_path));
        };
        job.record.status = CommentJobStatus::Skipped;
        job.record.error_message = Some("Rejected during review".to_string());
        job.telemetry_context = None;
        stored_run.run.updated_at = unix_timestamp_now();
        push_event(
            &mut stored_run.events,
            CommenterEventPayload {
                kind: CommenterEventKind::ReviewRejected,
                run_key: request.run_key,
                relative_path: Some(job.record.relative_path.clone()),
                level: EventLevel::Warn,
                message: "Review job rejected".to_string(),
                created_at: stored_run.run.updated_at,
            },
        );
        recalculate_run_counts(&mut stored_run.run, &stored_run.jobs);
        let detail = stored_run_detail(stored_run);
        persist_locked_state(&state)?;
        Ok(detail)
    }

    pub async fn retry_job(
        &self,
        app: Option<&AppHandle>,
        request: CommenterReviewActionRequest,
    ) -> Result<CommenterRunDetail, String> {
        {
            let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;
            let stored_run = state
                .runs
                .get_mut(&request.run_key)
                .ok_or_else(|| format!("unknown run: {}", request.run_key))?;
            let Some(job) = stored_run
                .jobs
                .iter_mut()
                .find(|job| job.record.relative_path == request.relative_path)
            else {
                return Err(format!("unknown job: {}", request.relative_path));
            };
            job.record.retry_count += 1;
            job.record.status = CommentJobStatus::Pending;
            job.record.error_message = None;
            job.telemetry_context = None;
            stored_run.run.updated_at = unix_timestamp_now();
            push_event(
                &mut stored_run.events,
                CommenterEventPayload {
                    kind: CommenterEventKind::JobUpdated,
                    run_key: request.run_key.clone(),
                    relative_path: Some(job.record.relative_path.clone()),
                    level: EventLevel::Info,
                    message: "Review job queued for retry".to_string(),
                    created_at: stored_run.run.updated_at,
                },
            );
            recalculate_run_counts(&mut stored_run.run, &stored_run.jobs);
            persist_locked_state(&state)?;
        }
        self.process_run(app, &request.run_key, true).await
    }

    pub fn open_external_diff(&self, request: CommenterReviewActionRequest) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;
        let command_template = state.diff_tool_settings.command_template.clone();
        let stored_run = state
            .runs
            .get_mut(&request.run_key)
            .ok_or_else(|| format!("unknown run: {}", request.run_key))?;
        let Some(job) = stored_run
            .jobs
            .iter()
            .find(|job| job.record.relative_path == request.relative_path)
        else {
            return Err(format!("unknown job: {}", request.relative_path));
        };
        let (Some(before_path), Some(candidate_path)) = (
            job.record.before_artifact_path.as_ref(),
            job.record.candidate_artifact_path.as_ref(),
        ) else {
            return Err("diff artifacts are not ready".to_string());
        };
        let rendered = render_diff_command(&command_template, before_path, candidate_path);
        launch_external_command(&rendered)?;
        push_event(
            &mut stored_run.events,
            CommenterEventPayload {
                kind: CommenterEventKind::ExternalDiffOpened,
                run_key: request.run_key,
                relative_path: Some(job.record.relative_path.clone()),
                level: EventLevel::Info,
                message: rendered,
                created_at: unix_timestamp_now(),
            },
        );
        persist_locked_state(&state)?;
        Ok(())
    }

    pub fn rollback_run(&self, run_key: &str) -> Result<CommenterRollbackSummary, String> {
        let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;
        let stored_run = state
            .runs
            .get_mut(run_key)
            .ok_or_else(|| format!("unknown run: {run_key}"))?;

        let mut rolled_back_files = Vec::new();
        let mut conflicted_files = Vec::new();

        for job in &mut stored_run.jobs {
            let Some(before_path_str) = &job.record.before_artifact_path else {
                continue;
            };
            let Some(before_hash) = &job.before_hash else {
                continue;
            };
            let Some(written_hash) = &job.written_hash else {
                continue;
            };

            let before_path = PathBuf::from(before_path_str);
            let before_content = match fs::read_to_string(&before_path) {
                Ok(content) => content,
                Err(_) => continue,
            };
            let current_content = match fs::read_to_string(&job.absolute_path) {
                Ok(content) => content,
                Err(_) => continue,
            };
            match can_overwrite_for_rollback(
                before_hash,
                written_hash,
                &hash_content(&current_content),
            ) {
                RollbackGuard::Safe => {
                    fs::write(&job.absolute_path, before_content)
                        .map_err(|error| error.to_string())?;
                    job.record.status = CommentJobStatus::RolledBack;
                    rolled_back_files.push(job.record.relative_path.clone());
                }
                RollbackGuard::AlreadyOriginal => {
                    rolled_back_files.push(job.record.relative_path.clone());
                }
                RollbackGuard::Conflict => {
                    conflicted_files.push(job.record.relative_path.clone());
                }
            }
        }

        stored_run.run.status = if conflicted_files.is_empty() {
            CommentRunStatus::RolledBack
        } else {
            CommentRunStatus::RollbackFailed
        };
        stored_run.run.updated_at = unix_timestamp_now();
        stored_run.run.finished_at = Some(stored_run.run.updated_at);
        push_event(
            &mut stored_run.events,
            CommenterEventPayload {
                kind: CommenterEventKind::RunRolledBack,
                run_key: run_key.to_string(),
                relative_path: None,
                level: if conflicted_files.is_empty() {
                    EventLevel::Info
                } else {
                    EventLevel::Warn
                },
                message: format!(
                    "Rollback completed: {} restored, {} conflicts",
                    rolled_back_files.len(),
                    conflicted_files.len()
                ),
                created_at: stored_run.run.updated_at,
            },
        );
        recalculate_run_counts(&mut stored_run.run, &stored_run.jobs);

        let summary = CommenterRollbackSummary {
            run_key: run_key.to_string(),
            rolled_back_files,
            conflicted_files,
        };
        persist_locked_state(&state)?;
        Ok(summary)
    }

    pub fn get_app_settings(&self) -> Result<CommenterRunSettingsView, String> {
        let state = self.state.lock().map_err(|_| "state lock poisoned")?;
        Ok(CommenterRunSettingsView {
            global_max_workers: state.app_settings.global_max_workers,
            api_concurrency_limit: state.app_settings.api_concurrency_limit,
            api_bearer_token: state.app_settings.api_bearer_token.clone(),
        })
    }

    pub fn update_app_settings(
        &self,
        request: CommenterRunSettingsView,
    ) -> Result<CommenterRunSettingsView, String> {
        let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;
        state.app_settings.global_max_workers = request.global_max_workers.max(1);
        state.app_settings.api_concurrency_limit = request.api_concurrency_limit.max(1);
        state.app_settings.api_bearer_token = request.api_bearer_token;
        let settings = CommenterRunSettingsView {
            global_max_workers: state.app_settings.global_max_workers,
            api_concurrency_limit: state.app_settings.api_concurrency_limit,
            api_bearer_token: state.app_settings.api_bearer_token.clone(),
        };
        persist_locked_state(&state)?;
        Ok(settings)
    }

    pub fn get_diff_tool_settings(&self) -> Result<CommenterDiffToolSettings, String> {
        let state = self.state.lock().map_err(|_| "state lock poisoned")?;
        Ok(state.diff_tool_settings.clone())
    }

    pub fn update_diff_tool_settings(
        &self,
        request: CommenterDiffToolSettings,
    ) -> Result<CommenterDiffToolSettings, String> {
        let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;
        state.diff_tool_settings = request.clone();
        persist_locked_state(&state)?;
        Ok(request)
    }

    async fn process_run(
        &self,
        app: Option<&AppHandle>,
        run_key: &str,
        from_resume: bool,
    ) -> Result<CommenterRunDetail, String> {
        // Phase 1：标记为 Running 并准备产物目录。期间锁只在此 block 内持有。
        let (run_paths, start_payload) = {
            let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;
            let stored_run = state
                .runs
                .get_mut(run_key)
                .ok_or_else(|| format!("unknown run: {run_key}"))?;
            let timestamp = unix_timestamp_now();
            stored_run.run.status = CommentRunStatus::Running;
            stored_run.run.started_at.get_or_insert(timestamp);
            stored_run.run.updated_at = timestamp;
            let start_payload = CommenterEventPayload {
                kind: if from_resume {
                    CommenterEventKind::RunResumed
                } else {
                    CommenterEventKind::RunStarted
                },
                run_key: run_key.to_string(),
                relative_path: None,
                level: EventLevel::Info,
                message: if from_resume {
                    "Run resumed".to_string()
                } else {
                    "Run started".to_string()
                },
                created_at: timestamp,
            };
            push_event(&mut stored_run.events, start_payload.clone());
            let data_root = state.data_root.clone();
            let run_paths =
                CommenterRunPaths::new(&data_root, run_key).map_err(|error| error.to_string())?;
            run_paths
                .create_directories()
                .map_err(|error| error.to_string())?;
            persist_locked_state(&state)?;
            (run_paths, start_payload)
        };
        emit_commenter_event(app, &start_payload);

        // Phase 2：按 worker 上限并发处理待办文件。
        let worker_limit = {
            let state = self.state.lock().map_err(|_| "state lock poisoned")?;
            let stored_run = state
                .runs
                .get(run_key)
                .ok_or_else(|| format!("unknown run: {run_key}"))?;
            stored_run
                .run
                .max_workers
                .max(1)
                .min(state.app_settings.global_max_workers.max(1))
                .min(state.app_settings.api_concurrency_limit.max(1)) as usize
        };
        let mut running_jobs = FuturesUnordered::new();
        let mut processed_files: i64 = 0;
        loop {
            while running_jobs.len() < worker_limit {
                // 选取下一个待处理任务。锁在 block 内短暂持有，绝不跨 await。
                let next_action = {
                    let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;
                    let stored_run = state
                        .runs
                        .get_mut(run_key)
                        .ok_or_else(|| format!("unknown run: {run_key}"))?;

                    // 暂停 / 取消信号：不再派发新任务，等待已派发任务自然收尾。
                    if matches!(
                        stored_run.run.status,
                        CommentRunStatus::Pausing
                            | CommentRunStatus::Paused
                            | CommentRunStatus::Cancelled
                            | CommentRunStatus::Failed
                    ) {
                        if stored_run.run.status == CommentRunStatus::Pausing {
                            stored_run.run.status = CommentRunStatus::Paused;
                            stored_run.run.updated_at = unix_timestamp_now();
                        }
                        None
                    } else if stored_run.run.max_files > 0
                        && processed_files >= stored_run.run.max_files
                    {
                        stored_run.run.status = CommentRunStatus::StoppedByLimit;
                        stored_run.run.updated_at = unix_timestamp_now();
                        None
                    } else {
                        let profile = state
                            .profiles
                            .get(
                                &state
                                    .runs
                                    .get(run_key)
                                    .map(|run| run.run.profile_key.clone())
                                    .unwrap_or_default(),
                            )
                            .cloned();
                        let app_settings = state.app_settings.clone();
                        let stored_run = state
                            .runs
                            .get_mut(run_key)
                            .ok_or_else(|| format!("unknown run: {run_key}"))?;
                        let Some(profile) = profile else {
                            return Err(format!("unknown profile: {}", stored_run.run.profile_key));
                        };

                        let next_index = stored_run.jobs.iter().position(|job| {
                            matches!(
                                job.record.status,
                                CommentJobStatus::Pending | CommentJobStatus::RetryWaiting
                            )
                        });
                        if let Some(index) = next_index {
                            let job = &mut stored_run.jobs[index];
                            let relative_path = job.record.relative_path.clone();
                            let absolute_path = job.absolute_path.clone();
                            let kind = job.kind.clone();
                            job.record.status = CommentJobStatus::Requesting;
                            stored_run.run.current_file = Some(relative_path.clone());
                            stored_run.run.updated_at = unix_timestamp_now();
                            let leased_payload = CommenterEventPayload {
                                kind: CommenterEventKind::JobUpdated,
                                run_key: run_key.to_string(),
                                relative_path: Some(relative_path.clone()),
                                level: EventLevel::Info,
                                message: format!("Processing file: {relative_path}"),
                                created_at: stored_run.run.updated_at,
                            };
                            push_event(&mut stored_run.events, leased_payload.clone());
                            let action = JobAction {
                                job_index: index,
                                relative_path,
                                absolute_path,
                                kind,
                                run_mode: stored_run.run.run_mode,
                                run_key: run_key.to_string(),
                                profile,
                                app_settings,
                            };
                            persist_locked_state(&state)?;
                            Some((action, leased_payload))
                        } else {
                            None
                        }
                    }
                };

                let Some((action, leased_payload)) = next_action else {
                    break;
                };
                emit_commenter_event(app, &leased_payload);
                processed_files += 1;
                let run_paths_for_job = run_paths.clone();
                let app_for_job = app.cloned();
                running_jobs.push(async move {
                    let outcome =
                        process_single_job_async(&action, &run_paths_for_job, app_for_job).await;
                    (action, outcome)
                });
            }

            let Some((action, outcome)) = running_jobs.next().await else {
                break;
            };

            // 回写结果。
            let event_payload = {
                let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;
                let stored_run = state
                    .runs
                    .get_mut(run_key)
                    .ok_or_else(|| format!("unknown run: {run_key}"))?;
                let payload = apply_job_outcome(stored_run, &action, outcome)?;
                recalculate_run_counts(&mut stored_run.run, &stored_run.jobs);
                stored_run.run.updated_at = unix_timestamp_now();
                persist_locked_state(&state)?;
                payload
            };

            if let Some(payload) = event_payload {
                emit_commenter_event(app, &payload);
            }
        }

        // Phase 3：终态。
        let detail = {
            let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;
            let stored_run = state
                .runs
                .get_mut(run_key)
                .ok_or_else(|| format!("unknown run: {run_key}"))?;
            stored_run.run.current_file = None;
            recalculate_run_counts(&mut stored_run.run, &stored_run.jobs);
            stored_run.run.updated_at = unix_timestamp_now();
            if !matches!(
                stored_run.run.status,
                CommentRunStatus::StoppedByLimit
                    | CommentRunStatus::Cancelled
                    | CommentRunStatus::Paused
                    | CommentRunStatus::Failed
            ) {
                stored_run.run.status = derive_terminal_status(&stored_run.run);
                if matches!(
                    stored_run.run.status,
                    CommentRunStatus::Completed | CommentRunStatus::CompletedWithIssues
                ) {
                    push_event(
                        &mut stored_run.events,
                        CommenterEventPayload {
                            kind: CommenterEventKind::RunCompleted,
                            run_key: run_key.to_string(),
                            relative_path: None,
                            level: EventLevel::Info,
                            message: format!("Run finished as {}", stored_run.run.status.as_str()),
                            created_at: stored_run.run.updated_at,
                        },
                    );
                }
            }
            stored_run.run.finished_at = match stored_run.run.status {
                CommentRunStatus::Completed
                | CommentRunStatus::CompletedWithIssues
                | CommentRunStatus::StoppedByLimit
                | CommentRunStatus::Cancelled => Some(stored_run.run.updated_at),
                _ => None,
            };
            let detail = stored_run_detail(stored_run);
            persist_locked_state(&state)?;
            detail
        };

        Ok(detail)
    }
}

fn default_state(data_root: PathBuf) -> CommenterState {
    CommenterState {
        data_root,
        next_profile_id: 1,
        next_run_id: 1,
        next_job_id: 1,
        app_settings: CommentAppSettings {
            global_max_workers: 2,
            api_concurrency_limit: 2,
            api_bearer_token: String::new(),
        },
        diff_tool_settings: CommenterDiffToolSettings {
            command_template: "code --diff \"{before}\" \"{after}\"".to_string(),
        },
        profiles: BTreeMap::new(),
        runs: BTreeMap::new(),
    }
}

fn load_state(data_root: PathBuf) -> CommenterState {
    let _ = fs::create_dir_all(&data_root);
    let snapshot_path = state_snapshot_path(&data_root);
    let mut state = fs::read_to_string(&snapshot_path)
        .ok()
        .and_then(|raw| serde_json::from_str::<CommenterState>(&raw).ok())
        .unwrap_or_else(|| default_state(data_root.clone()));
    state.data_root = data_root;
    let compacted = compact_transient_state(&mut state);
    recover_persisted_state(&mut state);
    if compacted {
        let _ = persist_locked_state(&state);
    }
    state
}

fn compact_transient_state(state: &mut CommenterState) -> bool {
    let mut changed = false;
    for stored_run in state.runs.values_mut() {
        let event_count = stored_run.events.len();
        stored_run
            .events
            .retain(|event| event.kind != CommenterEventKind::StreamChunk);
        changed |= stored_run.events.len() != event_count;

        for job in &mut stored_run.jobs {
            let artifact_exists = job
                .record
                .candidate_artifact_path
                .as_deref()
                .is_some_and(|path| Path::new(path).is_file());
            if artifact_exists && job.candidate_content.take().is_some() {
                changed = true;
            }
        }
    }
    changed
}

fn persist_locked_state(state: &CommenterState) -> Result<(), String> {
    fs::create_dir_all(&state.data_root).map_err(|error| error.to_string())?;
    let snapshot_path = state_snapshot_path(&state.data_root);
    let payload = serde_json::to_vec(state).map_err(|error| error.to_string())?;
    fs::write(snapshot_path, payload).map_err(|error| error.to_string())
}

fn state_snapshot_path(data_root: &Path) -> PathBuf {
    data_root.join(COMMENTER_STATE_FILE_NAME)
}

fn recover_persisted_state(state: &mut CommenterState) {
    for stored_run in state.runs.values_mut() {
        let recovery = recover_run_status(
            stored_run.run.status,
            stored_run
                .jobs
                .iter()
                .map(|job| job.record.status)
                .collect(),
        );
        if recovery.run_status == stored_run.run.status
            && recovery
                .job_statuses
                .iter()
                .zip(stored_run.jobs.iter())
                .all(|(status, job)| *status == job.record.status)
        {
            continue;
        }

        stored_run.run.status = recovery.run_status;
        stored_run.run.current_file = None;
        stored_run.run.updated_at = unix_timestamp_now();
        stored_run.run.finished_at = None;
        for (job, status) in stored_run
            .jobs
            .iter_mut()
            .zip(recovery.job_statuses.into_iter())
        {
            job.record.status = status;
        }
        push_event(
            &mut stored_run.events,
            CommenterEventPayload {
                kind: CommenterEventKind::JobUpdated,
                run_key: stored_run.run.run_key.clone(),
                relative_path: None,
                level: EventLevel::Warn,
                message: "Run recovered in paused state after restart".to_string(),
                created_at: stored_run.run.updated_at,
            },
        );
        recalculate_run_counts(&mut stored_run.run, &stored_run.jobs);
    }
}

fn render_diff_command(command_template: &str, before: &str, after: &str) -> String {
    command_template
        .replace("{before}", before)
        .replace("{after}", after)
}

fn launch_external_command(command_line: &str) -> Result<(), String> {
    let trimmed = command_line.trim();
    if trimmed.is_empty() {
        return Err("diff command template is empty".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", trimmed])
            .spawn()
            .map_err(|error| error.to_string())?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new("sh")
            .args(["-lc", trimmed])
            .spawn()
            .map_err(|error| error.to_string())?;
    }

    Ok(())
}

pub fn registered_command_names() -> &'static [&'static str] {
    COMMENTER_COMMAND_NAMES
}

fn build_stored_job(state: &mut CommenterState, file: ScannedFile) -> StoredJob {
    let job_id = state.next_job_id;
    state.next_job_id += 1;
    StoredJob {
        record: CommenterJobRecord {
            id: job_id,
            relative_path: file.relative_path,
            status: CommentJobStatus::Pending,
            language_hint: file.kind.language_hint.clone(),
            write_strategy: write_strategy_label(&file.kind.write_strategy).to_string(),
            retry_count: 0,
            error_message: None,
            before_artifact_path: None,
            candidate_artifact_path: None,
            sidecar_artifact_path: None,
        },
        absolute_path: file.absolute_path,
        kind: file.kind,
        candidate_content: None,
        before_hash: None,
        written_hash: None,
        telemetry_context: None,
    }
}

/// 单个文件流水线在锁外执行需要的全部入参。
#[derive(Debug, Clone)]
struct JobAction {
    job_index: usize,
    relative_path: String,
    absolute_path: PathBuf,
    kind: FileKind,
    run_mode: CommentRunMode,
    run_key: String,
    profile: CommenterProjectProfileView,
    app_settings: CommentAppSettings,
}

/// 单个文件流水线产出的结果，由调用方在持锁状态下回写到 StoredJob。
#[derive(Debug, Clone)]
enum JobOutcome {
    Skipped,
    Sidecar {
        sidecar_path: PathBuf,
        source: String,
    },
    AcceptedAuto {
        before_path: PathBuf,
        candidate_path: PathBuf,
        before_hash: String,
        written_hash: String,
        candidate: String,
        source: String,
    },
    ReviewReady {
        before_path: PathBuf,
        candidate_path: PathBuf,
        before_hash: String,
        candidate: String,
        source: String,
        telemetry_context: ChatCompletionsRequestContext,
    },
    ValidationRejected {
        before_path: PathBuf,
        candidate_path: PathBuf,
        before_hash: String,
        candidate: String,
        source: String,
        reason: String,
        telemetry_context: ChatCompletionsRequestContext,
    },
    Failed(String),
}

#[derive(Debug, Clone)]
struct JobResult {
    outcome: JobOutcome,
    events: Vec<CommenterEventPayload>,
}

impl JobResult {
    fn new(outcome: JobOutcome) -> Self {
        Self {
            outcome,
            events: Vec::new(),
        }
    }

    fn failed(reason: String) -> Self {
        Self::new(JobOutcome::Failed(reason))
    }
}

#[derive(Clone)]
struct GeneratedCandidate {
    result: Result<String, String>,
    events: Vec<CommenterEventPayload>,
    write_telemetry: Option<(CodebuddyTelemetry, ChatCompletionsRequestContext)>,
}

async fn process_single_job_async(
    action: &JobAction,
    run_paths: &CommenterRunPaths,
    app: Option<AppHandle>,
) -> JobResult {
    let source = match fs::read_to_string(&action.absolute_path) {
        Ok(content) => content,
        Err(error) => return JobResult::failed(format!("read source failed: {error}")),
    };

    match action.kind.write_strategy {
        WriteStrategy::Skip => return JobResult::new(JobOutcome::Skipped),
        WriteStrategy::SidecarOnly => {
            let sidecar_path = artifact_output_path(
                &run_paths.sidecar_root,
                &action.relative_path,
                ".commentary.txt",
            );
            return JobResult::new(JobOutcome::Sidecar {
                sidecar_path,
                source,
            });
        }
        WriteStrategy::AnnotateInPlace => {}
    }

    let generated = generate_candidate(action, &source, run_paths, app.as_ref()).await;
    let events = generated.events;
    let write_telemetry = generated.write_telemetry;
    let candidate = match generated.result {
        Ok(value) => value,
        Err(error) => {
            return JobResult {
                outcome: JobOutcome::Failed(error),
                events,
            }
        }
    };

    let validation = validate_candidate(
        FileValidationInput::source(&action.relative_path, &source),
        &candidate,
    );

    let before_path =
        artifact_output_path(&run_paths.before_root, &action.relative_path, ".before");
    let candidate_path = artifact_output_path(
        &run_paths.candidate_root,
        &action.relative_path,
        ".candidate",
    );
    if let Err(error) = write_with_parents(&before_path, &source) {
        return JobResult {
            outcome: JobOutcome::Failed(format!("write before snapshot failed: {error}")),
            events,
        };
    }
    if let Err(error) = write_with_parents(&candidate_path, &candidate) {
        return JobResult {
            outcome: JobOutcome::Failed(format!("write candidate failed: {error}")),
            events,
        };
    }

    let before_hash = hash_content(&source);

    match validation.decision {
        ValidationDecision::Accept => {
            if action.run_mode == CommentRunMode::Review {
                JobResult {
                    outcome: JobOutcome::ReviewReady {
                        before_path,
                        candidate_path,
                        before_hash,
                        candidate,
                        source,
                        telemetry_context: write_telemetry
                            .as_ref()
                            .expect("successful request telemetry")
                            .1
                            .clone(),
                    },
                    events,
                }
            } else {
                if let Err(error) = fs::write(&action.absolute_path, &candidate) {
                    return JobResult {
                        outcome: JobOutcome::Failed(format!("write source failed: {error}")),
                        events,
                    };
                }
                if let Some((telemetry, context)) = &write_telemetry {
                    telemetry.report_file_write(
                        context,
                        &action.relative_path,
                        &source,
                        &candidate,
                    );
                }
                let written_hash = hash_content(&candidate);
                JobResult {
                    outcome: JobOutcome::AcceptedAuto {
                        before_path,
                        candidate_path,
                        before_hash,
                        written_hash,
                        candidate,
                        source,
                    },
                    events,
                }
            }
        }
        ValidationDecision::Reject(reason) => JobResult {
            outcome: JobOutcome::ValidationRejected {
                before_path,
                candidate_path,
                before_hash,
                candidate,
                source,
                reason,
                telemetry_context: write_telemetry
                    .as_ref()
                    .expect("successful request telemetry")
                    .1
                    .clone(),
            },
            events,
        },
    }
}

async fn generate_candidate(
    action: &JobAction,
    source: &str,
    run_paths: &CommenterRunPaths,
    app: Option<&AppHandle>,
) -> GeneratedCandidate {
    let settings = &action.profile.settings;
    let prompt_template = action.profile.prompt_template.as_str();
    let app_settings = &action.app_settings;
    let credential_source = match describe_bearer_token_source(app_settings) {
        Ok(value) => value,
        Err(error) => {
            return GeneratedCandidate {
                result: Err(format!("credential resolution failed: {error:?}")),
                events: Vec::new(),
                write_telemetry: None,
            }
        }
    };

    let bearer_token = match resolve_bearer_token(app_settings) {
        Ok(token) => token,
        Err(error) => {
            return GeneratedCandidate {
                result: Err(format!("credential resolution failed: {error:?}")),
                events: Vec::new(),
                write_telemetry: None,
            }
        }
    };

    let parts: AnnotationPromptParts = build_annotation_prompt(
        prompt_template,
        action
            .kind
            .language_hint
            .as_deref()
            .unwrap_or(&action.kind.normalized_extension),
        &action.relative_path,
        source,
    );

    let base_url = if settings.api_base_url.trim().is_empty() {
        DEFAULT_API_BASE_URL.to_string()
    } else {
        settings.api_base_url.trim().to_string()
    };
    let model = if settings.api_model.trim().is_empty() {
        DEFAULT_API_MODEL.to_string()
    } else {
        settings.api_model.trim().to_string()
    };
    let timeout_secs = if settings.request_timeout_secs <= 0 {
        DEFAULT_REQUEST_TIMEOUT_SECS
    } else {
        settings.request_timeout_secs as u64
    };

    let request_context = ChatCompletionsRequestContext::new(&base_url);
    let request = ChatCompletionsRequest {
        base_url,
        bearer_token,
        model,
        system_prompt: parts.system,
        user_prompt: parts.user,
        max_tokens: DEFAULT_MAX_TOKENS,
        temperature: 1.0,
        timeout_secs,
        context: request_context,
    };
    let request_debug = build_chat_completions_request_debug(&request);
    let request_artifact_path = artifact_output_path(
        &run_paths.request_root,
        &action.relative_path,
        ".request.json",
    );
    if let Err(error) = write_json_artifact(&request_artifact_path, &request_debug) {
        return GeneratedCandidate {
            result: Err(format!("write request artifact failed: {error}")),
            events: Vec::new(),
            write_telemetry: None,
        };
    }
    let telemetry =
        CodebuddyTelemetry::new(&request.base_url, &request.bearer_token, &request.model);
    telemetry.report_chat_start(&request.context, &request.user_prompt, &request.user_prompt);
    let write_telemetry = Some((telemetry.clone(), request.context.clone()));
    let request_artifact_label = artifact_display_path(run_paths, &request_artifact_path);
    let response_artifact_path = artifact_output_path(
        &run_paths.response_root,
        &action.relative_path,
        ".response.json",
    );
    let response_artifact_label = artifact_display_path(run_paths, &response_artifact_path);

    let mut events = Vec::new();
    let request_started = job_event(
        action,
        CommenterEventKind::RequestStarted,
        EventLevel::Info,
        format!(
            "AI request started via {credential_source} -> {}; request artifact: {request_artifact_label}",
            request_debug.endpoint
        ),
    );
    emit_commenter_event(app, &request_started);
    events.push(request_started);

    let mut stream_buffer = String::new();
    let mut emitted_first_chunk = false;
    let mut last_stream_emit = Instant::now();
    let outcome = call_chat_completions_with_debug(request, |piece| {
        stream_buffer.push_str(piece);
        if !emitted_first_chunk
            || stream_buffer.chars().count() >= STREAM_EVENT_FLUSH_CHARS
            || last_stream_emit.elapsed() >= STREAM_EVENT_FLUSH_INTERVAL
        {
            emit_buffered_stream_events(action, app, &mut stream_buffer);
            emitted_first_chunk = true;
            last_stream_emit = Instant::now();
        }
    })
    .await;
    telemetry.report_chat_finish(
        &write_telemetry.as_ref().expect("telemetry context").1,
        &outcome.usage,
        outcome.result.is_ok(),
    );
    emit_buffered_stream_events(action, app, &mut stream_buffer);
    if let Err(error) = write_json_artifact(&response_artifact_path, &outcome.debug.response) {
        return GeneratedCandidate {
            result: Err(format!("write response artifact failed: {error}")),
            events,
            write_telemetry,
        };
    }

    let raw = match outcome.result {
        Ok(value) => value,
        Err(error) => {
            return GeneratedCandidate {
                result: Err(format!(
                    "{error} (response artifact: {response_artifact_label})"
                )),
                events,
                write_telemetry,
            }
        }
    };
    let response_status = outcome
        .debug
        .response
        .status
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let completed = job_event(
        action,
        CommenterEventKind::ModelResponseCompleted,
        EventLevel::Info,
        format!(
            "AI response completed: {} characters (HTTP {response_status}); response artifact: {response_artifact_label}",
            raw.chars().count()
        ),
    );
    emit_commenter_event(app, &completed);
    events.push(completed);

    let cleaned = sanitize_model_output(&raw);
    if cleaned.trim().is_empty() {
        return GeneratedCandidate {
            result: Err("model returned empty content".to_string()),
            events,
            write_telemetry,
        };
    }
    GeneratedCandidate {
        result: Ok(cleaned),
        events,
        write_telemetry,
    }
}

fn build_write_telemetry(
    profile: &CommenterProjectProfileView,
    app_settings: &CommentAppSettings,
) -> Result<(CodebuddyTelemetry, ChatCompletionsRequestContext), String> {
    let bearer_token = resolve_bearer_token(app_settings)
        .map_err(|error| format!("credential resolution failed: {error:?}"))?;
    let base_url = if profile.settings.api_base_url.trim().is_empty() {
        DEFAULT_API_BASE_URL
    } else {
        profile.settings.api_base_url.trim()
    };
    let model = if profile.settings.api_model.trim().is_empty() {
        DEFAULT_API_MODEL
    } else {
        profile.settings.api_model.trim()
    };
    Ok((
        CodebuddyTelemetry::new(base_url, &bearer_token, model),
        ChatCompletionsRequestContext::new(base_url),
    ))
}

fn job_event(
    action: &JobAction,
    kind: CommenterEventKind,
    level: EventLevel,
    message: String,
) -> CommenterEventPayload {
    CommenterEventPayload {
        kind,
        run_key: action.run_key.clone(),
        relative_path: Some(action.relative_path.clone()),
        level,
        message,
        created_at: unix_timestamp_now(),
    }
}

fn emit_buffered_stream_events(action: &JobAction, app: Option<&AppHandle>, buffer: &mut String) {
    if buffer.is_empty() {
        return;
    }

    let message = std::mem::take(buffer);
    let mut chunk = String::new();
    let mut chunk_chars = 0;
    for character in message.chars() {
        chunk.push(character);
        chunk_chars += 1;
        if chunk_chars == STREAM_EVENT_MAX_CHARS {
            emit_stream_event(action, app, std::mem::take(&mut chunk));
            chunk_chars = 0;
        }
    }
    if !chunk.is_empty() {
        emit_stream_event(action, app, chunk);
    }
}

fn emit_stream_event(action: &JobAction, app: Option<&AppHandle>, message: String) {
    let payload = job_event(
        action,
        CommenterEventKind::StreamChunk,
        EventLevel::Info,
        message,
    );
    emit_commenter_event(app, &payload);
}

/// 去掉模型偶尔附带的 ``` 围栏与首尾空行。
fn sanitize_model_output(raw: &str) -> String {
    let trimmed = raw.trim();
    let mut text = trimmed.to_string();

    if text.starts_with("```") {
        if let Some(first_newline) = text.find('\n') {
            text = text[first_newline + 1..].to_string();
        } else {
            text = String::new();
        }
    }
    if text.ends_with("```") {
        text.truncate(text.len() - 3);
    }
    text.trim_end().to_string() + "\n"
}

fn apply_job_outcome(
    stored_run: &mut StoredRun,
    action: &JobAction,
    result: JobResult,
) -> Result<Option<CommenterEventPayload>, String> {
    for payload in result.events {
        push_event(&mut stored_run.events, payload);
    }

    let Some(job) = stored_run.jobs.get_mut(action.job_index) else {
        return Err(format!("job index out of range: {}", action.job_index));
    };

    let timestamp = unix_timestamp_now();
    let event = match result.outcome {
        JobOutcome::Skipped => {
            job.record.status = CommentJobStatus::Skipped;
            Some(CommenterEventPayload {
                kind: CommenterEventKind::JobUpdated,
                run_key: action.run_key.clone(),
                relative_path: Some(action.relative_path.clone()),
                level: EventLevel::Info,
                message: "File skipped by classifier".to_string(),
                created_at: timestamp,
            })
        }
        JobOutcome::Sidecar {
            sidecar_path,
            source,
        } => {
            let sidecar_text =
                build_sidecar_content(&action.profile, &stored_run.run, job, &source);
            write_with_parents(&sidecar_path, &sidecar_text)?;
            job.record.sidecar_artifact_path = Some(sidecar_path.to_string_lossy().to_string());
            job.record.status = CommentJobStatus::Done;
            Some(CommenterEventPayload {
                kind: CommenterEventKind::JobUpdated,
                run_key: action.run_key.clone(),
                relative_path: Some(action.relative_path.clone()),
                level: EventLevel::Info,
                message: "Generated sidecar commentary".to_string(),
                created_at: timestamp,
            })
        }
        JobOutcome::AcceptedAuto {
            before_path,
            candidate_path,
            before_hash,
            written_hash,
            candidate: _candidate,
            source: _source,
        } => {
            job.record.before_artifact_path = Some(before_path.to_string_lossy().to_string());
            job.record.candidate_artifact_path = Some(candidate_path.to_string_lossy().to_string());
            job.before_hash = Some(before_hash);
            job.written_hash = Some(written_hash);
            job.candidate_content = None;
            job.record.status = CommentJobStatus::Done;
            Some(CommenterEventPayload {
                kind: CommenterEventKind::JobUpdated,
                run_key: action.run_key.clone(),
                relative_path: Some(action.relative_path.clone()),
                level: EventLevel::Info,
                message: "Candidate written to source".to_string(),
                created_at: timestamp,
            })
        }
        JobOutcome::ReviewReady {
            before_path,
            candidate_path,
            before_hash,
            candidate: _candidate,
            source: _source,
            telemetry_context,
        } => {
            job.record.before_artifact_path = Some(before_path.to_string_lossy().to_string());
            job.record.candidate_artifact_path = Some(candidate_path.to_string_lossy().to_string());
            job.before_hash = Some(before_hash);
            job.candidate_content = None;
            job.telemetry_context = Some(telemetry_context);
            job.record.status = CommentJobStatus::ReviewNeeded;
            Some(CommenterEventPayload {
                kind: CommenterEventKind::ReviewRequested,
                run_key: action.run_key.clone(),
                relative_path: Some(action.relative_path.clone()),
                level: EventLevel::Info,
                message: "Candidate ready for review".to_string(),
                created_at: timestamp,
            })
        }
        JobOutcome::ValidationRejected {
            before_path,
            candidate_path,
            before_hash,
            candidate: _candidate,
            source: _source,
            reason,
            telemetry_context,
        } => {
            job.record.before_artifact_path = Some(before_path.to_string_lossy().to_string());
            job.record.candidate_artifact_path = Some(candidate_path.to_string_lossy().to_string());
            job.before_hash = Some(before_hash);
            job.candidate_content = None;
            job.telemetry_context = Some(telemetry_context);
            job.record.status = CommentJobStatus::ReviewNeeded;
            job.record.error_message = Some(reason.clone());
            Some(CommenterEventPayload {
                kind: CommenterEventKind::ReviewRequested,
                run_key: action.run_key.clone(),
                relative_path: Some(action.relative_path.clone()),
                level: EventLevel::Warn,
                message: format!("Candidate requires review: {reason}"),
                created_at: timestamp,
            })
        }
        JobOutcome::Failed(reason) => {
            // 若仍在重试预算内，转 RetryWaiting；否则 failed。
            if job.record.retry_count < stored_run.run.max_retries {
                job.record.retry_count += 1;
                job.record.status = CommentJobStatus::RetryWaiting;
                // RetryWaiting 在循环里会被重新拣取（process_run 里 next_index 包含了 RetryWaiting）。
                // 立即将其转回 Pending 让循环可见；保留 retry_count 防止无限重试。
                job.record.status = CommentJobStatus::Pending;
            } else {
                job.record.status = CommentJobStatus::Failed;
            }
            job.record.error_message = Some(reason.clone());
            Some(CommenterEventPayload {
                kind: CommenterEventKind::JobFailed,
                run_key: action.run_key.clone(),
                relative_path: Some(action.relative_path.clone()),
                level: EventLevel::Error,
                message: reason,
                created_at: timestamp,
            })
        }
    };

    if let Some(payload) = event.clone() {
        push_event(&mut stored_run.events, payload);
    }
    Ok(event)
}

fn recalculate_run_counts(run: &mut CommenterRunRecord, jobs: &[StoredJob]) {
    run.total_jobs = jobs.len() as i64;
    run.completed_jobs = jobs
        .iter()
        .filter(|job| {
            matches!(
                job.record.status,
                CommentJobStatus::Done | CommentJobStatus::RolledBack
            )
        })
        .count() as i64;
    run.review_needed_jobs = jobs
        .iter()
        .filter(|job| job.record.status == CommentJobStatus::ReviewNeeded)
        .count() as i64;
    run.failed_jobs = jobs
        .iter()
        .filter(|job| job.record.status == CommentJobStatus::Failed)
        .count() as i64;
    run.skipped_jobs = jobs
        .iter()
        .filter(|job| job.record.status == CommentJobStatus::Skipped)
        .count() as i64;
    run.pending_jobs = jobs
        .iter()
        .filter(|job| {
            matches!(
                job.record.status,
                CommentJobStatus::Pending
                    | CommentJobStatus::Leased
                    | CommentJobStatus::Requesting
                    | CommentJobStatus::Validating
                    | CommentJobStatus::Writing
                    | CommentJobStatus::RetryWaiting
            )
        })
        .count() as i64;
}

fn derive_terminal_status(run: &CommenterRunRecord) -> CommentRunStatus {
    if run.failed_jobs > 0 || run.review_needed_jobs > 0 {
        CommentRunStatus::CompletedWithIssues
    } else {
        CommentRunStatus::Completed
    }
}

fn stored_run_detail(stored_run: &StoredRun) -> CommenterRunDetail {
    CommenterRunDetail {
        run: stored_run.run.clone(),
        jobs: stored_run
            .jobs
            .iter()
            .map(|job| job.record.clone())
            .collect(),
        events: stored_run
            .events
            .iter()
            .filter(|event| event.kind != CommenterEventKind::StreamChunk)
            .cloned()
            .collect(),
    }
}

fn write_before_snapshot(
    run_paths: &CommenterRunPaths,
    relative_path: &str,
    content: &str,
    job: &mut StoredJob,
) -> Result<(), String> {
    let before_path = artifact_output_path(&run_paths.before_root, relative_path, ".before");
    write_with_parents(&before_path, content)?;
    job.record.before_artifact_path = Some(before_path.to_string_lossy().to_string());
    job.before_hash = Some(hash_content(content));
    Ok(())
}

fn build_sidecar_content(
    profile: &CommenterProjectProfileView,
    run: &CommenterRunRecord,
    job: &StoredJob,
    source: &str,
) -> String {
    format!(
        "Profile: {}\nRun: {}\nFile: {}\nStrategy: {}\n\nSummary:\nThis file was preserved in-place because it uses sidecar-only handling.\n\nPreview:\n{}",
        profile.profile_name,
        run.run_key,
        job.record.relative_path,
        job.record.write_strategy,
        source.lines().take(12).collect::<Vec<_>>().join("\n")
    )
}

fn artifact_output_path(root: &Path, relative_path: &str, suffix: &str) -> PathBuf {
    let mut path = root.join(relative_path);
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("artifact");
    path.set_file_name(format!("{file_name}{suffix}"));
    path
}

fn read_artifact_text(
    root: &Path,
    relative_path: &str,
    suffix: &str,
    escape_error: &str,
) -> Result<String, String> {
    let artifact_path = artifact_output_path(root, relative_path, suffix);
    let canonical_root = match root.canonicalize() {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(error) => return Err(error.to_string()),
    };
    let canonical_target = match artifact_path.canonicalize() {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(error) => return Err(error.to_string()),
    };
    if !canonical_target.starts_with(&canonical_root) {
        return Err(escape_error.to_string());
    }

    match std::fs::read_to_string(&canonical_target) {
        Ok(content) => Ok(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error.to_string()),
    }
}

fn artifact_display_path(run_paths: &CommenterRunPaths, artifact_path: &Path) -> String {
    artifact_path
        .strip_prefix(&run_paths.run_root)
        .unwrap_or(artifact_path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn write_with_parents(path: &Path, content: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    fs::write(path, content).map_err(|error| error.to_string())
}

fn write_json_artifact<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let content = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    write_with_parents(path, &content)
}

fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn normalize_extensions(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect()
}

fn push_event(events: &mut Vec<CommenterEventPayload>, event: CommenterEventPayload) {
    events.push(event);
}

fn write_strategy_label(strategy: &WriteStrategy) -> &'static str {
    match strategy {
        WriteStrategy::AnnotateInPlace => "annotate_in_place",
        WriteStrategy::SidecarOnly => "sidecar_only",
        WriteStrategy::Skip => "skip",
    }
}

fn unix_timestamp_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_upsert_project_profile(
    surface: tauri::State<'_, CommenterCommandSurface>,
    request: CommenterProjectProfileDraft,
) -> Result<CommenterProjectProfileView, String> {
    surface.upsert_project_profile(request)
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_list_project_profiles(
    surface: tauri::State<'_, CommenterCommandSurface>,
) -> Result<Vec<CommenterProjectProfileView>, String> {
    surface.list_project_profiles()
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_delete_project_profile(
    surface: tauri::State<'_, CommenterCommandSurface>,
    project_key: String,
) -> Result<CommenterProjectProfileView, String> {
    surface.delete_project_profile(&project_key)
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_enqueue_run(
    surface: tauri::State<'_, CommenterCommandSurface>,
    request: CommenterEnqueueRunRequest,
) -> Result<CommenterRunHandle, String> {
    surface.enqueue_run(request)
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_list_runs(
    surface: tauri::State<'_, CommenterCommandSurface>,
) -> Result<Vec<CommenterRunRecord>, String> {
    surface.list_runs()
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_get_run_detail(
    surface: tauri::State<'_, CommenterCommandSurface>,
    run_key: String,
) -> Result<Option<CommenterRunDetail>, String> {
    surface.get_run_detail(&run_key)
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_delete_run(
    surface: tauri::State<'_, CommenterCommandSurface>,
    run_id: String,
) -> Result<CommenterRunRecord, String> {
    surface.delete_run(&run_id)
}

#[cfg(not(test))]
#[tauri::command]
pub async fn commenter_start_run(
    app: AppHandle,
    surface: tauri::State<'_, CommenterCommandSurface>,
    run_id: String,
) -> Result<CommenterRunDetail, String> {
    surface.start_run(Some(&app), &run_id).await
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_pause_run(
    surface: tauri::State<'_, CommenterCommandSurface>,
    run_id: String,
) -> Result<CommenterRunDetail, String> {
    surface.pause_run(&run_id)
}

#[cfg(not(test))]
#[tauri::command]
pub async fn commenter_resume_run(
    app: AppHandle,
    surface: tauri::State<'_, CommenterCommandSurface>,
    run_id: String,
) -> Result<CommenterRunDetail, String> {
    surface.resume_run(Some(&app), &run_id).await
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_cancel_run(
    surface: tauri::State<'_, CommenterCommandSurface>,
    run_id: String,
) -> Result<CommenterRunDetail, String> {
    surface.cancel_run(&run_id)
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_list_review_jobs(
    surface: tauri::State<'_, CommenterCommandSurface>,
) -> Result<Vec<CommenterJobRecord>, String> {
    surface.list_review_jobs()
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_accept_review_job(
    surface: tauri::State<'_, CommenterCommandSurface>,
    request: CommenterReviewActionRequest,
) -> Result<CommenterRunDetail, String> {
    surface.accept_review_job(request)
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_reject_review_job(
    surface: tauri::State<'_, CommenterCommandSurface>,
    request: CommenterReviewActionRequest,
) -> Result<CommenterRunDetail, String> {
    surface.reject_review_job(request)
}

#[cfg(not(test))]
#[tauri::command]
pub async fn commenter_retry_job(
    app: AppHandle,
    surface: tauri::State<'_, CommenterCommandSurface>,
    request: CommenterReviewActionRequest,
) -> Result<CommenterRunDetail, String> {
    surface.retry_job(Some(&app), request).await
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_open_external_diff(
    surface: tauri::State<'_, CommenterCommandSurface>,
    request: CommenterReviewActionRequest,
) -> Result<(), String> {
    surface.open_external_diff(request)
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_rollback_run(
    surface: tauri::State<'_, CommenterCommandSurface>,
    run_id: String,
) -> Result<CommenterRollbackSummary, String> {
    surface.rollback_run(&run_id)
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_get_app_settings(
    surface: tauri::State<'_, CommenterCommandSurface>,
) -> Result<CommenterRunSettingsView, String> {
    surface.get_app_settings()
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_update_app_settings(
    surface: tauri::State<'_, CommenterCommandSurface>,
    request: CommenterRunSettingsView,
) -> Result<CommenterRunSettingsView, String> {
    surface.update_app_settings(request)
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_get_diff_tool_settings(
    surface: tauri::State<'_, CommenterCommandSurface>,
) -> Result<CommenterDiffToolSettings, String> {
    surface.get_diff_tool_settings()
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_update_diff_tool_settings(
    surface: tauri::State<'_, CommenterCommandSurface>,
    request: CommenterDiffToolSettings,
) -> Result<CommenterDiffToolSettings, String> {
    surface.update_diff_tool_settings(request)
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_list_dir(
    surface: tauri::State<'_, CommenterCommandSurface>,
    profile_key: String,
    relative_path: String,
) -> Result<Vec<CommenterDirEntry>, String> {
    surface.list_dir(&profile_key, &relative_path)
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_get_candidate_text(
    surface: tauri::State<'_, CommenterCommandSurface>,
    run_key: String,
    relative_path: String,
) -> Result<String, String> {
    surface.get_candidate_text(&run_key, &relative_path)
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_get_original_text(
    surface: tauri::State<'_, CommenterCommandSurface>,
    run_key: String,
    relative_path: String,
) -> Result<String, String> {
    surface.get_original_text(&run_key, &relative_path)
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_get_data_paths(
    surface: tauri::State<'_, CommenterCommandSurface>,
) -> Result<CommenterDataPaths, String> {
    surface.data_paths()
}

#[cfg(not(test))]
#[tauri::command]
pub fn commenter_open_path(
    surface: tauri::State<'_, CommenterCommandSurface>,
    path: String,
) -> Result<(), String> {
    let target = PathBuf::from(&path);
    let data_root = surface.data_root();
    let canonical_target = match target.canonicalize() {
        Ok(value) => value,
        Err(_) => {
            // Target may not exist yet (e.g. artifacts dir created lazily). Create it
            // when it lives under the managed data root, then re-canonicalize.
            if target.starts_with(&data_root) {
                std::fs::create_dir_all(&target).map_err(|error| error.to_string())?;
                target.canonicalize().map_err(|error| error.to_string())?
            } else {
                return Err(format!("path is outside the managed data root: {path}"));
            }
        }
    };
    let canonical_root = data_root
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if !canonical_target.starts_with(&canonical_root) {
        return Err(format!("path is outside the managed data root: {path}"));
    }

    let reveal_target: PathBuf = if canonical_target.is_dir() {
        canonical_target.clone()
    } else {
        canonical_target
            .parent()
            .map(|value| value.to_path_buf())
            .unwrap_or_else(|| canonical_target.clone())
    };

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(reveal_target)
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(reveal_target)
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(reveal_target)
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    fn draft(root_path: &str) -> CommenterProjectProfileDraft {
        draft_with_api(root_path, "", "")
    }

    fn draft_with_api(
        root_path: &str,
        api_base_url: &str,
        _api_bearer_token: &str,
    ) -> CommenterProjectProfileDraft {
        CommenterProjectProfileDraft {
            project_key: "demo".to_string(),
            profile_name: "Demo".to_string(),
            root_path: root_path.to_string(),
            include_extensions: vec!["go".to_string(), "json".to_string()],
            exclude_directories: vec!["node_modules".to_string()],
            prompt_template: "annotate".to_string(),
            settings: CommentProjectSettings {
                credential_profile_key: "token".to_string(),
                default_run_mode: CommentRunMode::Review,
                default_max_workers: 2,
                default_max_retries: 1,
                default_max_files: 10,
                allow_light_rewrite: true,
                json_handling_strategy: JsonHandlingStrategy::SidecarOnly,
                api_base_url: api_base_url.to_string(),
                api_model: DEFAULT_API_MODEL.to_string(),
                api_bearer_token: String::new(),
                request_timeout_secs: 600,
            },
        }
    }

    fn configure_global_api_token(service: &CommenterCommandSurface, api_bearer_token: &str) {
        service
            .update_app_settings(CommenterRunSettingsView {
                global_max_workers: 2,
                api_concurrency_limit: 2,
                api_bearer_token: api_bearer_token.to_string(),
            })
            .expect("global api token");
    }

    fn spawn_sse_server(content: &'static str) -> String {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            thread,
        };

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = [0_u8; 4096];
            let _ = stream.read(&mut buffer);
            let body = format!(
                "data: {{\"choices\":[{{\"delta\":{{\"reasoning_content\":\"忽略\"}}}}]}}\n\n\
data: {{\"choices\":[{{\"delta\":{{\"content\":{content:?}}}}}]}}\n\n\
data: [DONE]\n\n"
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.as_bytes().len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write sse response");
        });
        format!("http://{}", addr)
    }

    fn spawn_counting_sse_server(
        expected_requests: usize,
        delay_ms: u64,
        content: &'static str,
    ) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            sync::{
                atomic::{AtomicUsize, Ordering},
                Arc,
            },
            thread,
            time::Duration,
        };

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind counting server");
        let addr = listener.local_addr().expect("local addr");
        let in_flight = Arc::new(AtomicUsize::new(0));
        let peak_in_flight = Arc::new(AtomicUsize::new(0));
        let peak_for_return = peak_in_flight.clone();

        thread::spawn(move || {
            for _ in 0..expected_requests {
                let (mut stream, _) = listener.accept().expect("accept request");
                let in_flight = in_flight.clone();
                let peak_in_flight = peak_in_flight.clone();
                thread::spawn(move || {
                    let mut buffer = [0_u8; 4096];
                    let _ = stream.read(&mut buffer);
                    let current = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                    peak_in_flight.fetch_max(current, Ordering::SeqCst);
                    thread::sleep(Duration::from_millis(delay_ms));
                    let body = format!(
                        "data: {{\"choices\":[{{\"delta\":{{\"content\":{content:?}}}}}]}}\n\n\
data: [DONE]\n\n"
                    );
                    let response = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                        body.as_bytes().len(),
                        body
                    );
                    stream
                        .write_all(response.as_bytes())
                        .expect("write sse response");
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });

        (format!("http://{}", addr), peak_for_return)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn enqueue_and_start_run_generates_review_jobs_and_sidecars() {
        let temp = tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        fs::create_dir_all(project_root.join("src")).expect("src dir");
        fs::write(
            project_root.join("src/main.go"),
            "package main\nfunc main() {}\n",
        )
        .expect("go");
        fs::write(project_root.join("src/config.json"), "{\"ok\":true}\n").expect("json");

        let service = CommenterCommandSurface::new(temp.path().join(".commenter-data"));
        let api_base_url = spawn_sse_server("// main 入口函数\npackage main\nfunc main() {}\n");
        configure_global_api_token(&service, "test-token");
        service
            .upsert_project_profile(draft_with_api(
                project_root.to_string_lossy().as_ref(),
                &api_base_url,
                "test-token",
            ))
            .expect("profile");
        let handle = service
            .enqueue_run(CommenterEnqueueRunRequest {
                profile_key: "demo".to_string(),
                requested_by: Some("test".to_string()),
                run_mode: CommentRunMode::Review,
                max_workers: 2,
                max_retries: 1,
                max_files: 10,
                allow_light_rewrite: true,
                json_handling_strategy: JsonHandlingStrategy::SidecarOnly,
            })
            .expect("run");
        let detail = service
            .start_run(None, &handle.run_key)
            .await
            .expect("start run");

        assert_eq!(detail.run.review_needed_jobs, 1);
        assert_eq!(detail.run.completed_jobs, 1);
        assert!(detail
            .jobs
            .iter()
            .any(|job| job.sidecar_artifact_path.is_some()));
        assert!(detail
            .jobs
            .iter()
            .any(|job| job.candidate_artifact_path.is_some()));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn accept_review_then_rollback_restores_written_file() {
        let temp = tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        fs::create_dir_all(project_root.join("src")).expect("src dir");
        let source_path = project_root.join("src/main.go");
        fs::write(&source_path, "package main\nfunc main() {}\n").expect("go");

        let service = CommenterCommandSurface::new(temp.path().join(".commenter-data"));
        let api_base_url = spawn_sse_server("// main 入口函数\npackage main\nfunc main() {}\n");
        configure_global_api_token(&service, "test-token");
        service
            .upsert_project_profile(draft_with_api(
                project_root.to_string_lossy().as_ref(),
                &api_base_url,
                "test-token",
            ))
            .expect("profile");
        let handle = service
            .enqueue_run(CommenterEnqueueRunRequest {
                profile_key: "demo".to_string(),
                requested_by: Some("test".to_string()),
                run_mode: CommentRunMode::Review,
                max_workers: 2,
                max_retries: 1,
                max_files: 10,
                allow_light_rewrite: true,
                json_handling_strategy: JsonHandlingStrategy::SidecarOnly,
            })
            .expect("run");
        let detail = service
            .start_run(None, &handle.run_key)
            .await
            .expect("start run");
        let review_job = detail
            .jobs
            .iter()
            .find(|job| job.status == CommentJobStatus::ReviewNeeded)
            .expect("review job");

        service
            .accept_review_job(CommenterReviewActionRequest {
                run_key: handle.run_key.clone(),
                relative_path: review_job.relative_path.clone(),
            })
            .expect("accept");
        let written = fs::read_to_string(&source_path).expect("written source");
        assert!(written.contains("入口函数"));

        let rollback = service.rollback_run(&handle.run_key).expect("rollback");
        let restored = fs::read_to_string(&source_path).expect("restored source");
        assert!(rollback.conflicted_files.is_empty());
        assert_eq!(restored, "package main\nfunc main() {}\n");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn missing_credential_fails_job_instead_of_building_stub_candidate() {
        let temp = tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        fs::create_dir_all(project_root.join("src")).expect("src dir");
        let source_path = project_root.join("src/main.go");
        fs::write(&source_path, "package main\nfunc main() {}\n").expect("go");

        let service = CommenterCommandSurface::new(temp.path().join(".commenter-data"));
        service
            .upsert_project_profile(draft(project_root.to_string_lossy().as_ref()))
            .expect("profile");
        let handle = service
            .enqueue_run(CommenterEnqueueRunRequest {
                profile_key: "demo".to_string(),
                requested_by: Some("test".to_string()),
                run_mode: CommentRunMode::Review,
                max_workers: 2,
                max_retries: 0,
                max_files: 10,
                allow_light_rewrite: true,
                json_handling_strategy: JsonHandlingStrategy::SidecarOnly,
            })
            .expect("run");

        let detail = service
            .start_run(None, &handle.run_key)
            .await
            .expect("start run");
        let written = fs::read_to_string(&source_path).expect("source remains");

        assert_eq!(detail.run.failed_jobs, 1);
        assert_eq!(detail.run.review_needed_jobs, 0);
        assert_eq!(written, "package main\nfunc main() {}\n");
        assert!(detail.jobs.iter().any(|job| job
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("credential")));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn start_run_uses_configured_workers_for_http_jobs() {
        use std::sync::atomic::Ordering;

        let temp = tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        fs::create_dir_all(project_root.join("src")).expect("src dir");
        fs::write(
            project_root.join("src/first.go"),
            "package main\nfunc main() {}\n",
        )
        .expect("first");
        fs::write(
            project_root.join("src/second.go"),
            "package main\nfunc main() {}\n",
        )
        .expect("second");

        let service = CommenterCommandSurface::new(temp.path().join(".commenter-data"));
        let (api_base_url, peak_in_flight) =
            spawn_counting_sse_server(2, 200, "// main 入口函数\npackage main\nfunc main() {}\n");
        configure_global_api_token(&service, "test-token");
        service
            .upsert_project_profile(draft_with_api(
                project_root.to_string_lossy().as_ref(),
                &api_base_url,
                "test-token",
            ))
            .expect("profile");
        let handle = service
            .enqueue_run(CommenterEnqueueRunRequest {
                profile_key: "demo".to_string(),
                requested_by: Some("test".to_string()),
                run_mode: CommentRunMode::Review,
                max_workers: 2,
                max_retries: 0,
                max_files: 10,
                allow_light_rewrite: true,
                json_handling_strategy: JsonHandlingStrategy::SidecarOnly,
            })
            .expect("run");

        let detail = service
            .start_run(None, &handle.run_key)
            .await
            .expect("start run");

        assert_eq!(detail.run.review_needed_jobs, 2);
        assert!(
            peak_in_flight.load(Ordering::SeqCst) >= 2,
            "HTTP jobs should overlap when max_workers is 2"
        );
    }

    #[test]
    fn queued_run_can_be_deleted_before_start() {
        let temp = tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        fs::create_dir_all(project_root.join("src")).expect("src dir");
        fs::write(
            project_root.join("src/main.go"),
            "package main\nfunc main() {}\n",
        )
        .expect("go");

        let service = CommenterCommandSurface::new(temp.path().join(".commenter-data"));
        service
            .upsert_project_profile(draft(project_root.to_string_lossy().as_ref()))
            .expect("profile");
        let handle = service
            .enqueue_run(CommenterEnqueueRunRequest {
                profile_key: "demo".to_string(),
                requested_by: Some("test".to_string()),
                run_mode: CommentRunMode::Review,
                max_workers: 2,
                max_retries: 0,
                max_files: 10,
                allow_light_rewrite: true,
                json_handling_strategy: JsonHandlingStrategy::SidecarOnly,
            })
            .expect("run");

        let deleted = service.delete_run(&handle.run_key).expect("delete run");

        assert_eq!(deleted.run_key, handle.run_key);
        assert!(service
            .get_run_detail(&handle.run_key)
            .expect("get deleted run")
            .is_none());
        assert!(service.list_runs().expect("list runs").is_empty());
    }

    #[test]
    fn project_profile_delete_is_safe_for_run_references() {
        let temp = tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        fs::create_dir_all(project_root.join("src")).expect("src dir");
        fs::write(
            project_root.join("src/main.go"),
            "package main\nfunc main() {}\n",
        )
        .expect("go");

        let service = CommenterCommandSurface::new(temp.path().join(".commenter-data"));
        service
            .upsert_project_profile(draft(project_root.to_string_lossy().as_ref()))
            .expect("profile");
        let handle = service
            .enqueue_run(CommenterEnqueueRunRequest {
                profile_key: "demo".to_string(),
                requested_by: Some("test".to_string()),
                run_mode: CommentRunMode::Review,
                max_workers: 2,
                max_retries: 0,
                max_files: 10,
                allow_light_rewrite: true,
                json_handling_strategy: JsonHandlingStrategy::SidecarOnly,
            })
            .expect("run");

        let error = service
            .delete_project_profile("demo")
            .expect_err("referenced profile must remain");
        assert!(error.contains("referenced by a run"));
        service.delete_run(&handle.run_key).expect("delete run");
        let deleted = service
            .delete_project_profile("demo")
            .expect("delete unreferenced profile");
        assert_eq!(deleted.project_key, "demo");
        assert!(service
            .list_project_profiles()
            .expect("profiles")
            .is_empty());
    }
    #[tokio::test(flavor = "current_thread")]
    async fn start_run_keeps_stream_chunks_transient() {
        let temp = tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        fs::create_dir_all(project_root.join("src")).expect("src dir");
        fs::write(
            project_root.join("src/main.go"),
            "package main\nfunc main() {}\n",
        )
        .expect("go");

        let service = CommenterCommandSurface::new(temp.path().join(".commenter-data"));
        let api_base_url = spawn_sse_server("// main comment\npackage main\nfunc main() {}\n");
        configure_global_api_token(&service, "test-token");
        service
            .upsert_project_profile(draft_with_api(
                project_root.to_string_lossy().as_ref(),
                &api_base_url,
                "test-token",
            ))
            .expect("profile");
        let handle = service
            .enqueue_run(CommenterEnqueueRunRequest {
                profile_key: "demo".to_string(),
                requested_by: Some("test".to_string()),
                run_mode: CommentRunMode::Review,
                max_workers: 1,
                max_retries: 0,
                max_files: 10,
                allow_light_rewrite: true,
                json_handling_strategy: JsonHandlingStrategy::SidecarOnly,
            })
            .expect("run");

        let detail = service
            .start_run(None, &handle.run_key)
            .await
            .expect("start run");

        assert!(detail
            .events
            .iter()
            .any(|event| event.kind == CommenterEventKind::RequestStarted));
        assert!(detail
            .events
            .iter()
            .all(|event| event.kind != CommenterEventKind::StreamChunk));
        assert!(detail
            .events
            .iter()
            .any(|event| event.kind == CommenterEventKind::ModelResponseCompleted));
        let snapshot = fs::read_to_string(state_snapshot_path(&service.data_root()))
            .expect("read compact state snapshot");
        assert!(!snapshot.contains("stream_chunk"));
        assert!(!snapshot.contains("candidate_content"));
    }

    #[test]
    fn load_state_compacts_legacy_stream_events_and_artifact_backed_candidates() {
        let temp = tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        fs::create_dir_all(project_root.join("src")).expect("src dir");
        fs::write(project_root.join("src/main.go"), "package main\n").expect("go");
        let data_root = temp.path().join(".commenter-data");
        let service = CommenterCommandSurface::new(&data_root);
        service
            .upsert_project_profile(draft(project_root.to_string_lossy().as_ref()))
            .expect("profile");
        let handle = service
            .enqueue_run(CommenterEnqueueRunRequest {
                profile_key: "demo".to_string(),
                requested_by: Some("test".to_string()),
                run_mode: CommentRunMode::Review,
                max_workers: 1,
                max_retries: 0,
                max_files: 10,
                allow_light_rewrite: true,
                json_handling_strategy: JsonHandlingStrategy::SidecarOnly,
            })
            .expect("run");
        let candidate_path = data_root.join("legacy-candidate.txt");
        fs::write(&candidate_path, "artifact candidate").expect("candidate artifact");

        {
            let mut state = service.state.lock().expect("state");
            let stored_run = state.runs.get_mut(&handle.run_key).expect("stored run");
            stored_run.events.push(CommenterEventPayload {
                kind: CommenterEventKind::StreamChunk,
                run_key: handle.run_key.clone(),
                relative_path: Some("src/main.go".to_string()),
                level: EventLevel::Info,
                message: "legacy streamed payload".to_string(),
                created_at: unix_timestamp_now(),
            });
            stored_run.jobs[0].candidate_content = Some("duplicate candidate".to_string());
            stored_run.jobs[0].record.candidate_artifact_path =
                Some(candidate_path.to_string_lossy().to_string());
            persist_locked_state(&state).expect("persist legacy state");
        }
        drop(service);

        let reloaded = CommenterCommandSurface::new(&data_root);
        let detail = reloaded
            .get_run_detail(&handle.run_key)
            .expect("detail")
            .expect("stored run");
        assert!(detail
            .events
            .iter()
            .all(|event| event.kind != CommenterEventKind::StreamChunk));
        let state = reloaded.state.lock().expect("state");
        assert!(state.runs[&handle.run_key].jobs[0]
            .candidate_content
            .is_none());
        drop(state);
        let snapshot = fs::read_to_string(state_snapshot_path(&data_root)).expect("snapshot");
        assert!(!snapshot.contains("legacy streamed payload"));
        assert!(!snapshot.contains("duplicate candidate"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn start_run_writes_http_debug_artifacts_and_redacts_bearer_token() {
        let temp = tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        fs::create_dir_all(project_root.join("src")).expect("src dir");
        fs::write(
            project_root.join("src/main.go"),
            "package main\nfunc main() {}\n",
        )
        .expect("go");

        let service = CommenterCommandSurface::new(temp.path().join(".commenter-data"));
        let api_base_url = spawn_sse_server("// main comment\npackage main\nfunc main() {}\n");
        configure_global_api_token(&service, "test-token-123456");
        service
            .upsert_project_profile(draft_with_api(
                project_root.to_string_lossy().as_ref(),
                &api_base_url,
                "test-token-123456",
            ))
            .expect("profile");
        let handle = service
            .enqueue_run(CommenterEnqueueRunRequest {
                profile_key: "demo".to_string(),
                requested_by: Some("test".to_string()),
                run_mode: CommentRunMode::Review,
                max_workers: 1,
                max_retries: 0,
                max_files: 10,
                allow_light_rewrite: true,
                json_handling_strategy: JsonHandlingStrategy::SidecarOnly,
            })
            .expect("run");

        let detail = service
            .start_run(None, &handle.run_key)
            .await
            .expect("start run");
        let run_paths = crate::commenter::artifacts::CommenterRunPaths::new(
            &service.data_root(),
            &handle.run_key,
        )
        .expect("run paths");
        let request_artifact = run_paths
            .request_root
            .join("src")
            .join("main.go.request.json");
        let response_artifact = run_paths
            .response_root
            .join("src")
            .join("main.go.response.json");

        let request_raw = std::fs::read_to_string(&request_artifact).expect("request artifact");
        let response_raw = std::fs::read_to_string(&response_artifact).expect("response artifact");

        assert!(request_raw.contains("\"Authorization\""));
        assert!(
            !request_raw.contains("test-token-123456"),
            "request artifact must redact raw bearer token"
        );
        assert!(response_raw.contains("\"status\": 200"));
        assert!(response_raw.contains("text/event-stream"));
        assert!(response_raw.contains("data: {"));
        assert!(detail.events.iter().any(|event| {
            event.kind == CommenterEventKind::RequestStarted
                && event.message.contains("request artifact")
        }));
        assert!(detail.events.iter().any(|event| {
            event.kind == CommenterEventKind::ModelResponseCompleted
                && event.message.contains("response artifact")
        }));
    }

    #[test]
    fn command_surface_initializes_sqlite_app_database() {
        let temp = tempdir().expect("tempdir");
        let data_root = temp.path().join(".commenter-data");

        let _service = CommenterCommandSurface::new(&data_root);

        let db_path = data_root.join("app.db");
        assert!(db_path.exists(), "runtime app.db should be created");

        let conn = rusqlite::Connection::open(db_path).expect("open app.db");
        let version: String = conn
            .query_row(
                "SELECT value FROM commenter_schema_meta WHERE key = 'version'",
                [],
                |row| row.get(0),
            )
            .expect("schema version");
        assert_eq!(version, "9");
    }

    #[test]
    fn list_dir_returns_entries_under_profile_root_and_filters_excludes() {
        let temp = tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(project_root.join("src")).unwrap();
        std::fs::create_dir_all(project_root.join("node_modules")).unwrap();
        std::fs::write(project_root.join("src").join("a.ts"), "x").unwrap();
        std::fs::write(project_root.join("README.md"), "x").unwrap();

        let service = CommenterCommandSurface::new(temp.path().join(".commenter-data"));
        let profile = service
            .upsert_project_profile(CommenterProjectProfileDraft {
                project_key: "demo".into(),
                profile_name: "demo".into(),
                root_path: project_root.to_string_lossy().into_owned(),
                include_extensions: vec!["ts".into()],
                exclude_directories: vec!["node_modules".into()],
                prompt_template: String::new(),
                settings: CommentProjectSettings {
                    credential_profile_key: String::new(),
                    default_run_mode: CommentRunMode::Review,
                    default_max_workers: 2,
                    default_max_retries: 1,
                    default_max_files: 10,
                    allow_light_rewrite: false,
                    json_handling_strategy: JsonHandlingStrategy::SidecarOnly,
                    api_base_url: String::new(),
                    api_model: "glm-5.0".to_string(),
                    api_bearer_token: String::new(),
                    request_timeout_secs: 600,
                },
            })
            .expect("upsert profile");

        let entries = service
            .list_dir(&profile.project_key, "")
            .expect("list root");
        let names: std::collections::HashSet<String> =
            entries.iter().map(|e| e.name.clone()).collect();
        assert!(names.contains("src"), "src directory present");
        assert!(names.contains("README.md"), "README at root present");
        assert!(
            !names.contains("node_modules"),
            "excluded directory must be filtered: {:?}",
            names
        );
    }

    #[test]
    fn list_dir_rejects_path_outside_profile_root() {
        let temp = tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).unwrap();

        let service = CommenterCommandSurface::new(temp.path().join(".commenter-data"));
        let profile = service
            .upsert_project_profile(CommenterProjectProfileDraft {
                project_key: "demo2".into(),
                profile_name: "demo2".into(),
                root_path: project_root.to_string_lossy().into_owned(),
                include_extensions: vec!["ts".into()],
                exclude_directories: vec![],
                prompt_template: String::new(),
                settings: CommentProjectSettings {
                    credential_profile_key: String::new(),
                    default_run_mode: CommentRunMode::Review,
                    default_max_workers: 2,
                    default_max_retries: 1,
                    default_max_files: 10,
                    allow_light_rewrite: false,
                    json_handling_strategy: JsonHandlingStrategy::SidecarOnly,
                    api_base_url: String::new(),
                    api_model: "glm-5.0".to_string(),
                    api_bearer_token: String::new(),
                    request_timeout_secs: 600,
                },
            })
            .expect("upsert profile");

        let outcome = service.list_dir(&profile.project_key, "../../../etc");
        assert!(
            outcome.is_err(),
            "path traversal must be rejected, got {:?}",
            outcome
        );
    }

    #[test]
    fn get_candidate_text_returns_artifact_content() {
        let temp = tempfile::tempdir().expect("tempdir");
        let data_root = temp.path().join(".commenter-data");
        let service = CommenterCommandSurface::new(&data_root);

        let run_key = "demo-run";
        let run_paths = crate::commenter::artifacts::CommenterRunPaths::new(&data_root, run_key)
            .expect("run paths");
        run_paths.create_directories().expect("mkdirs");

        let candidate_path = run_paths.candidate_root.join("src").join("a.ts.candidate");
        std::fs::create_dir_all(candidate_path.parent().unwrap()).unwrap();
        std::fs::write(&candidate_path, "// generated\nexport {};\n").unwrap();

        let text = service
            .get_candidate_text(run_key, "src/a.ts")
            .expect("read candidate");
        assert!(text.contains("// generated"));
    }

    #[test]
    fn get_candidate_text_falls_back_to_sidecar_artifact() {
        let temp = tempfile::tempdir().expect("tempdir");
        let data_root = temp.path().join(".commenter-data");
        let service = CommenterCommandSurface::new(&data_root);

        let run_key = "demo-run";
        let run_paths = crate::commenter::artifacts::CommenterRunPaths::new(&data_root, run_key)
            .expect("run paths");
        run_paths.create_directories().expect("mkdirs");

        let sidecar_path = run_paths
            .sidecar_root
            .join("src")
            .join("payload.json.commentary.txt");
        std::fs::create_dir_all(sidecar_path.parent().unwrap()).unwrap();
        std::fs::write(&sidecar_path, "sidecar preview\n").unwrap();

        let text = service
            .get_candidate_text(run_key, "src/payload.json")
            .expect("read sidecar");
        assert_eq!(text, "sidecar preview\n");
    }

    #[test]
    fn get_candidate_text_returns_empty_when_artifact_missing() {
        let temp = tempfile::tempdir().expect("tempdir");
        let data_root = temp.path().join(".commenter-data");
        let service = CommenterCommandSurface::new(&data_root);

        let text = service
            .get_candidate_text("nonexistent-run", "missing/file.ts")
            .expect("missing artifact returns Ok");
        assert_eq!(text, "");
    }

    #[test]
    fn get_original_text_returns_before_artifact_content() {
        let temp = tempfile::tempdir().expect("tempdir");
        let data_root = temp.path().join(".commenter-data");
        let service = CommenterCommandSurface::new(&data_root);

        let run_key = "demo-run";
        let run_paths = crate::commenter::artifacts::CommenterRunPaths::new(&data_root, run_key)
            .expect("run paths");
        run_paths.create_directories().expect("mkdirs");

        let before_path = run_paths.before_root.join("src").join("a.ts.before");
        std::fs::create_dir_all(before_path.parent().unwrap()).unwrap();
        std::fs::write(&before_path, "// original\nexport {};\n").unwrap();

        let text = service
            .get_original_text(run_key, "src/a.ts")
            .expect("read original");
        assert_eq!(text, "// original\nexport {};\n");
    }
}
