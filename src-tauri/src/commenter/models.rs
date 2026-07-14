#![allow(dead_code)]

//! Domain models for the AI comment orchestrator.

use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub const COMMENT_RUN_STATUS_VALUES: &[&str] = &[
    "queued",
    "scanning",
    "ready",
    "running",
    "pausing",
    "paused",
    "stopped_by_limit",
    "completed",
    "completed_with_issues",
    "cancelled",
    "failed",
    "rollback_ready",
    "rolled_back",
    "rollback_failed",
];

pub const COMMENT_JOB_STATUS_VALUES: &[&str] = &[
    "pending",
    "leased",
    "requesting",
    "validating",
    "writing",
    "done",
    "review_needed",
    "retry_waiting",
    "failed",
    "skipped",
    "rolled_back",
];

pub const COMMENT_CREDENTIAL_SOURCE_KIND_VALUES: &[&str] =
    &["env_var", "inline_secret", "json_file"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentRunStatus {
    Queued,
    Scanning,
    Ready,
    Running,
    Pausing,
    Paused,
    StoppedByLimit,
    Completed,
    CompletedWithIssues,
    Cancelled,
    Failed,
    RollbackReady,
    RolledBack,
    RollbackFailed,
}

impl CommentRunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Scanning => "scanning",
            Self::Ready => "ready",
            Self::Running => "running",
            Self::Pausing => "pausing",
            Self::Paused => "paused",
            Self::StoppedByLimit => "stopped_by_limit",
            Self::Completed => "completed",
            Self::CompletedWithIssues => "completed_with_issues",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
            Self::RollbackReady => "rollback_ready",
            Self::RolledBack => "rolled_back",
            Self::RollbackFailed => "rollback_failed",
        }
    }
}

impl FromStr for CommentRunStatus {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "queued" => Ok(Self::Queued),
            "scanning" => Ok(Self::Scanning),
            "ready" => Ok(Self::Ready),
            "running" => Ok(Self::Running),
            "pausing" => Ok(Self::Pausing),
            "paused" => Ok(Self::Paused),
            "stopped_by_limit" => Ok(Self::StoppedByLimit),
            "completed" => Ok(Self::Completed),
            "completed_with_issues" => Ok(Self::CompletedWithIssues),
            "cancelled" => Ok(Self::Cancelled),
            "failed" => Ok(Self::Failed),
            "rollback_ready" => Ok(Self::RollbackReady),
            "rolled_back" => Ok(Self::RolledBack),
            "rollback_failed" => Ok(Self::RollbackFailed),
            _ => Err("invalid comment run status"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentJobStatus {
    Pending,
    Leased,
    Requesting,
    Validating,
    Writing,
    Done,
    ReviewNeeded,
    RetryWaiting,
    Failed,
    Skipped,
    RolledBack,
}

impl CommentJobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Leased => "leased",
            Self::Requesting => "requesting",
            Self::Validating => "validating",
            Self::Writing => "writing",
            Self::Done => "done",
            Self::ReviewNeeded => "review_needed",
            Self::RetryWaiting => "retry_waiting",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::RolledBack => "rolled_back",
        }
    }
}

impl FromStr for CommentJobStatus {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pending" => Ok(Self::Pending),
            "leased" => Ok(Self::Leased),
            "requesting" => Ok(Self::Requesting),
            "validating" => Ok(Self::Validating),
            "writing" => Ok(Self::Writing),
            "done" => Ok(Self::Done),
            "review_needed" => Ok(Self::ReviewNeeded),
            "retry_waiting" => Ok(Self::RetryWaiting),
            "failed" => Ok(Self::Failed),
            "skipped" => Ok(Self::Skipped),
            "rolled_back" => Ok(Self::RolledBack),
            _ => Err("invalid comment job status"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentRunMode {
    Auto,
    Review,
}

impl CommentRunMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Review => "review",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialSourceKind {
    EnvVar,
    InlineSecret,
    JsonFile,
}

impl CredentialSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EnvVar => "env_var",
            Self::InlineSecret => "inline_secret",
            Self::JsonFile => "json_file",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JsonHandlingStrategy {
    SidecarOnly,
}

impl JsonHandlingStrategy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SidecarOnly => "sidecar_only",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentAppSettings {
    pub global_max_workers: i64,
    pub api_concurrency_limit: i64,
    #[serde(default)]
    pub api_bearer_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentCredentialProfile {
    pub id: i64,
    pub profile_key: String,
    pub display_name: String,
    pub source_kind: CredentialSourceKind,
    pub source_reference: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentProjectSettings {
    #[serde(default)]
    pub credential_profile_key: String,
    pub default_run_mode: CommentRunMode,
    pub default_max_workers: i64,
    pub default_max_retries: i64,
    pub default_max_files: i64,
    pub allow_light_rewrite: bool,
    pub json_handling_strategy: JsonHandlingStrategy,
    #[serde(default = "default_api_base_url")]
    pub api_base_url: String,
    #[serde(default = "default_api_model")]
    pub api_model: String,
    #[serde(default)]
    pub api_bearer_token: String,
    #[serde(default = "default_request_timeout_secs")]
    pub request_timeout_secs: i64,
}

pub fn default_api_base_url() -> String {
    "https://unvcoding.copilot.qq.com".to_string()
}

pub fn default_api_model() -> String {
    "glm-5.1".to_string()
}

pub fn default_request_timeout_secs() -> i64 {
    600
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentRunSettingsSnapshot {
    pub credential_profile_key: String,
    pub run_mode: CommentRunMode,
    pub max_workers: i64,
    pub max_retries: i64,
    pub max_files: i64,
    pub allow_light_rewrite: bool,
    pub json_handling_strategy: JsonHandlingStrategy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentArtifactKind {
    InputSnapshot,
    ModelResponse,
    OutputPatch,
    LogBundle,
}

impl CommentArtifactKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InputSnapshot => "input_snapshot",
            Self::ModelResponse => "model_response",
            Self::OutputPatch => "output_patch",
            Self::LogBundle => "log_bundle",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventLevel {
    Info,
    Warn,
    Error,
}

impl EventLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentProjectProfile {
    pub id: i64,
    pub project_key: String,
    pub root_path: String,
    pub profile_name: String,
    pub prompt_template: String,
    pub include_globs_json: Option<String>,
    pub exclude_globs_json: Option<String>,
    pub settings_json: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentQueueRun {
    pub id: i64,
    pub profile_id: i64,
    pub run_key: String,
    pub status: CommentRunStatus,
    pub requested_by: Option<String>,
    pub total_jobs: i64,
    pub completed_jobs: i64,
    pub run_settings_json: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentFileJob {
    pub id: i64,
    pub run_id: i64,
    pub relative_path: String,
    pub status: CommentJobStatus,
    pub language_hint: Option<String>,
    pub retry_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentArtifact {
    pub id: i64,
    pub run_id: i64,
    pub file_job_id: Option<i64>,
    pub kind: CommentArtifactKind,
    pub storage_path: String,
    pub byte_size: i64,
    pub sha256: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentRunEvent {
    pub id: i64,
    pub run_id: i64,
    pub file_job_id: Option<i64>,
    pub level: EventLevel,
    pub event_type: String,
    pub message: String,
    pub created_at: i64,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn run_statuses_match_approved_design_strings() {
        let approved = [
            "queued",
            "scanning",
            "ready",
            "running",
            "pausing",
            "paused",
            "stopped_by_limit",
            "completed",
            "completed_with_issues",
            "cancelled",
            "failed",
            "rollback_ready",
            "rolled_back",
            "rollback_failed",
        ];

        let parsed: Vec<CommentRunStatus> = serde_json::from_value(json!(approved)).expect("parse");
        let actual: Vec<&str> = parsed.into_iter().map(CommentRunStatus::as_str).collect();
        assert_eq!(actual, approved);
    }

    #[test]
    fn job_statuses_match_approved_design_strings() {
        let approved = [
            "pending",
            "leased",
            "requesting",
            "validating",
            "writing",
            "done",
            "review_needed",
            "retry_waiting",
            "failed",
            "skipped",
            "rolled_back",
        ];

        let parsed: Vec<CommentJobStatus> = serde_json::from_value(json!(approved)).expect("parse");
        let actual: Vec<&str> = parsed.into_iter().map(CommentJobStatus::as_str).collect();
        assert_eq!(actual, approved);
    }

    #[test]
    fn app_settings_capture_global_limits() {
        let settings: CommentAppSettings = serde_json::from_value(json!({
            "global_max_workers": 12,
            "api_concurrency_limit": 6,
            "api_bearer_token": "Bearer test-token"
        }))
        .expect("parse app settings");

        assert_eq!(settings.global_max_workers, 12);
        assert_eq!(settings.api_concurrency_limit, 6);
        assert_eq!(settings.api_bearer_token, "Bearer test-token");
    }

    #[test]
    fn project_settings_dto_captures_minimum_task_one_fields() {
        let settings: CommentProjectSettings = serde_json::from_value(json!({
            "default_run_mode": "review",
            "default_max_workers": 4,
            "default_max_retries": 2,
            "default_max_files": 50,
            "allow_light_rewrite": true,
            "json_handling_strategy": "sidecar_only"
        }))
        .expect("parse settings");

        assert_eq!(settings.credential_profile_key, "");
        assert_eq!(settings.default_run_mode.as_str(), "review");
        assert_eq!(settings.default_max_workers, 4);
        assert_eq!(settings.default_max_retries, 2);
        assert_eq!(settings.default_max_files, 50);
        assert!(settings.allow_light_rewrite);
        assert_eq!(settings.json_handling_strategy.as_str(), "sidecar_only");
        assert_eq!(settings.api_model, "glm-5.1");
    }

    #[test]
    fn credential_profiles_store_opaque_inline_secret_handles() {
        let profile: CommentCredentialProfile = serde_json::from_value(json!({
            "id": 1,
            "profile_key": "ui-inline",
            "display_name": "UI Inline Secret",
            "source_kind": "inline_secret",
            "source_reference": "secret://credential/ui-inline",
            "created_at": 1,
            "updated_at": 2
        }))
        .expect("parse credential profile");

        assert_eq!(profile.profile_key, "ui-inline");
        assert_eq!(profile.source_kind.as_str(), "inline_secret");
        assert_eq!(profile.source_reference, "secret://credential/ui-inline");

        let encoded = serde_json::to_value(&profile).expect("encode credential profile");
        assert_eq!(encoded["source_reference"], "secret://credential/ui-inline");
        assert!(encoded.get("inline_secret").is_none());
    }

    #[test]
    fn run_settings_snapshot_round_trips_as_non_secret_json() {
        let snapshot = CommentRunSettingsSnapshot {
            credential_profile_key: "team-default".to_string(),
            run_mode: CommentRunMode::Auto,
            max_workers: 8,
            max_retries: 3,
            max_files: 200,
            allow_light_rewrite: false,
            json_handling_strategy: JsonHandlingStrategy::SidecarOnly,
        };

        let encoded = serde_json::to_value(&snapshot).expect("encode");
        let decoded: CommentRunSettingsSnapshot =
            serde_json::from_value(encoded.clone()).expect("decode");

        assert_eq!(decoded, snapshot);
        assert_eq!(encoded["credential_profile_key"], "team-default");
        assert_eq!(encoded["run_mode"], "auto");
        assert_eq!(encoded["json_handling_strategy"], "sidecar_only");
        assert!(encoded.get("credential_source").is_none());
        assert!(!encoded.to_string().contains("secret-token"));
    }

    #[test]
    fn artifact_kinds_match_sql_strings() {
        let approved = [
            "input_snapshot",
            "model_response",
            "output_patch",
            "log_bundle",
        ];

        let parsed: Vec<CommentArtifactKind> =
            serde_json::from_value(json!(approved)).expect("parse");
        let actual: Vec<&str> = parsed
            .into_iter()
            .map(CommentArtifactKind::as_str)
            .collect();
        assert_eq!(actual, approved);
    }

    #[test]
    fn event_levels_match_sql_strings() {
        let approved = ["info", "warn", "error"];

        let parsed: Vec<EventLevel> = serde_json::from_value(json!(approved)).expect("parse");
        let actual: Vec<&str> = parsed.into_iter().map(EventLevel::as_str).collect();
        assert_eq!(actual, approved);
    }
}
