use serde::{Deserialize, Serialize};
#[cfg(not(test))]
use tauri::{AppHandle, Emitter};

use super::models::EventLevel;

pub const COMMENTER_EVENT_CHANNEL: &str = "commenter://state";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommenterEventKind {
    RunQueued,
    RunStarted,
    RunPaused,
    RunResumed,
    RunCancelled,
    RunCompleted,
    JobUpdated,
    JobFailed,
    RequestStarted,
    StreamChunk,
    ModelResponseCompleted,
    ReviewRequested,
    ReviewAccepted,
    ReviewRejected,
    RunRolledBack,
    ExternalDiffOpened,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommenterEventPayload {
    pub kind: CommenterEventKind,
    pub run_key: String,
    pub relative_path: Option<String>,
    pub level: EventLevel,
    pub message: String,
    pub created_at: i64,
}

#[cfg(not(test))]
pub fn emit_commenter_event(app: Option<&AppHandle>, payload: &CommenterEventPayload) {
    if let Some(handle) = app {
        let _ = handle.emit(COMMENTER_EVENT_CHANNEL, payload);
    }
}

#[cfg(test)]
pub fn emit_commenter_event<T>(_app: Option<&T>, _payload: &CommenterEventPayload) {}
