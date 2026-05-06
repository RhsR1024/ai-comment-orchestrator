# AI Comment Orchestrator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Tauri desktop tool that queues multiple projects, annotates files through the verified chat-completions API, supports review and rollback, and survives interruption safely.

**Architecture:** Add a dedicated Rust `commenter` domain under `src-tauri/src/commenter/` for persistence, scanning, HTTP/SSE execution, scheduling, artifact storage, review, and rollback. Expose thin Tauri commands/events through `src/lib/tauri.ts`, then build a Vue tool page plus focused components that follow the project’s existing `reactive` store pattern instead of introducing a new frontend state library.

**Tech Stack:** Tauri 2, Rust, rusqlite, reqwest, tokio, Vue 3, TypeScript, Vue Router, lucide-vue-next, existing `src/lib/tauri.ts` invoke layer, existing `src/locales/messages.ts` i18n structure

---

## File Structure

- Modify: `src-tauri/Cargo.toml`
  Responsibility: add any missing crates needed for SSE parsing and secure-ish local storage helpers only if current dependencies are insufficient.
- Modify: `src-tauri/src/main.rs`
  Responsibility: register the new `commenter` module, app state, commands, and events.
- Create: `src-tauri/src/commenter/mod.rs`
  Responsibility: feature module root and public re-exports.
- Create: `src-tauri/src/commenter/models.rs`
  Responsibility: define run/job/settings/event DTOs and enum state machines.
- Create: `src-tauri/src/commenter/db.rs`
  Responsibility: create SQLite schema, migrations, and CRUD helpers for profiles, runs, jobs, artifacts, review actions, and rollback actions.
- Create: `src-tauri/src/commenter/config.rs`
  Responsibility: resolve credential profiles, runtime config snapshots, and data-root helpers.
- Create: `src-tauri/src/commenter/scanner.rs`
  Responsibility: scan project trees, apply file filters, classify file types, and create manifest rows.
- Create: `src-tauri/src/commenter/prompt.rs`
  Responsibility: build language-family prompts and `.json` sidecar prompts.
- Create: `src-tauri/src/commenter/http.rs`
  Responsibility: build outbound request payloads and headers for the verified API.
- Create: `src-tauri/src/commenter/sse.rs`
  Responsibility: parse SSE streams and collect only `delta.content`.
- Create: `src-tauri/src/commenter/validate.rs`
  Responsibility: normalize model output and apply strong/soft validation rules.
- Create: `src-tauri/src/commenter/artifacts.rs`
  Responsibility: write `before`, `candidate`, `sidecar`, request, and response artifacts under the run directory.
- Create: `src-tauri/src/commenter/rollback.rs`
  Responsibility: track writable snapshots and perform run-level rollback with conflict detection.
- Create: `src-tauri/src/commenter/scheduler.rs`
  Responsibility: own worker-pool scheduling, pause/resume/cancel, retry backoff, and recovery on restart.
- Create: `src-tauri/src/commenter/events.rs`
  Responsibility: define Tauri event payloads and emit helpers.
- Create: `src-tauri/src/commenter/commands.rs`
  Responsibility: expose thin Tauri commands for profiles, queueing, run control, review actions, diff launch, and rollback.
- Create: `src/lib/commenterTypes.ts`
  Responsibility: mirror Rust DTOs for the frontend.
- Create: `src/lib/commenterStore.ts`
  Responsibility: frontend reactive store for queue/runs/review/history state.
- Create: `src/lib/commenterView.ts`
  Responsibility: lightweight computed helpers for badges, summaries, and progress bars.
- Modify: `src/lib/tauri.ts`
  Responsibility: add typed wrappers for new commands.
- Modify: `src/router/index.ts`
  Responsibility: register the new tool route.
- Modify: `src/components/Sidebar.vue`
  Responsibility: add the new tool entry to the existing tools navigation pattern.
- Modify: `src/pages/ToolsHubPage.vue`
  Responsibility: add the new tool card to the tools hub.
- Modify: `src/locales/messages.ts`
  Responsibility: add all user-facing strings.
- Create: `src/pages/CommentOrchestratorPage.vue`
  Responsibility: top-level tool page and section switcher.
- Create: `src/components/commenter/ProjectProfilesPanel.vue`
  Responsibility: project profile CRUD form.
- Create: `src/components/commenter/QueueRunsTable.vue`
  Responsibility: queue overview table and run controls.
- Create: `src/components/commenter/RunDetailPanel.vue`
  Responsibility: current run metrics, worker state, and event timeline.
- Create: `src/components/commenter/ReviewJobsPanel.vue`
  Responsibility: review-needed list and accept/reject/retry actions.
- Create: `src/components/commenter/RunHistoryPanel.vue`
  Responsibility: run history and rollback actions.
- Create: `src/components/commenter/DiffToolSettingsPanel.vue`
  Responsibility: external diff tool templates and test launch controls.

## Task 1: Define Rust Domain Types And SQLite Schema

**Files:**
- Create: `src-tauri/src/commenter/mod.rs`
- Create: `src-tauri/src/commenter/models.rs`
- Create: `src-tauri/src/commenter/db.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_profiles_runs_and_jobs_tables() {
        let conn = open_in_memory().expect("in-memory db");
        migrate(&conn).expect("migrate");

        let tables = list_table_names(&conn).expect("table list");
        assert!(tables.contains(&"commenter_project_profiles".to_string()));
        assert!(tables.contains(&"commenter_queue_runs".to_string()));
        assert!(tables.contains(&"commenter_file_jobs".to_string()));
        assert!(tables.contains(&"commenter_artifacts".to_string()));
        assert!(tables.contains(&"commenter_run_events".to_string()));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test commenter::db::tests::creates_profiles_runs_and_jobs_tables --manifest-path src-tauri/Cargo.toml`

Expected: FAIL with `commenter` module or `migrate` function not found

- [ ] **Step 3: Write minimal implementation**

```rust
// src-tauri/src/commenter/mod.rs
pub mod db;
pub mod models;
```

```rust
// src-tauri/src/commenter/db.rs
use rusqlite::{Connection, Result};

pub fn open_in_memory() -> Result<Connection> {
    Connection::open_in_memory()
}

pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        create table if not exists commenter_project_profiles (
          project_id text primary key,
          display_name text not null,
          project_root text not null,
          created_at text not null,
          updated_at text not null
        );
        create table if not exists commenter_queue_runs (
          run_id text primary key,
          project_id text not null,
          status text not null,
          created_at text not null
        );
        create table if not exists commenter_file_jobs (
          job_id text primary key,
          run_id text not null,
          relative_path text not null,
          status text not null,
          attempt_count integer not null default 0
        );
        create table if not exists commenter_artifacts (
          job_id text primary key,
          before_path text,
          candidate_path text,
          sidecar_path text
        );
        create table if not exists commenter_run_events (
          event_id text primary key,
          run_id text not null,
          level text not null,
          message text not null,
          created_at text not null
        );
        "#,
    )?;
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test commenter::db::tests::creates_profiles_runs_and_jobs_tables --manifest-path src-tauri/Cargo.toml`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commenter/mod.rs src-tauri/src/commenter/models.rs src-tauri/src/commenter/db.rs src-tauri/src/main.rs
git commit -m "feat(commenter): add base models and sqlite schema"
```

## Task 2: Add Config Resolution And Artifact Layout

**Files:**
- Create: `src-tauri/src/commenter/config.rs`
- Create: `src-tauri/src/commenter/artifacts.rs`
- Modify: `src-tauri/src/commenter/models.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn builds_run_directory_structure_under_data_dir() {
    let temp = tempfile::tempdir().expect("tempdir");
    let run_paths = CommenterRunPaths::new(temp.path(), "run-123").expect("paths");

    assert!(run_paths.run_root.ends_with("run-123"));
    assert!(run_paths.before_root.ends_with("before"));
    assert!(run_paths.candidate_root.ends_with("candidates"));
    assert!(run_paths.sidecar_root.ends_with("sidecars"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test commenter::artifacts::tests::builds_run_directory_structure_under_data_dir --manifest-path src-tauri/Cargo.toml`

Expected: FAIL with `CommenterRunPaths` not found

- [ ] **Step 3: Write minimal implementation**

```rust
// src-tauri/src/commenter/artifacts.rs
use std::path::{Path, PathBuf};

pub struct CommenterRunPaths {
    pub run_root: PathBuf,
    pub before_root: PathBuf,
    pub candidate_root: PathBuf,
    pub sidecar_root: PathBuf,
    pub request_root: PathBuf,
    pub response_root: PathBuf,
}

impl CommenterRunPaths {
    pub fn new(data_root: &Path, run_id: &str) -> Result<Self, String> {
        let run_root = data_root.join("commenter").join("runs").join(run_id);
        Ok(Self {
            before_root: run_root.join("before"),
            candidate_root: run_root.join("candidates"),
            sidecar_root: run_root.join("sidecars"),
            request_root: run_root.join("request"),
            response_root: run_root.join("response"),
            run_root,
        })
    }
}
```

```rust
// src-tauri/src/commenter/config.rs
pub enum CredentialSource {
    EnvVar(String),
    InlineSecret(String),
    JsonFile { path: String, key: String },
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test commenter::artifacts::tests::builds_run_directory_structure_under_data_dir --manifest-path src-tauri/Cargo.toml`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commenter/config.rs src-tauri/src/commenter/artifacts.rs src-tauri/src/commenter/models.rs src-tauri/src/main.rs
git commit -m "feat(commenter): add config resolution and artifact layout"
```

## Task 3: Build Scanner, Prompt Builder, HTTP Client, And SSE Parser

**Files:**
- Create: `src-tauri/src/commenter/scanner.rs`
- Create: `src-tauri/src/commenter/prompt.rs`
- Create: `src-tauri/src/commenter/http.rs`
- Create: `src-tauri/src/commenter/sse.rs`
- Modify: `src-tauri/src/commenter/models.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn scanner_classifies_json_as_sidecar_only() {
    let kind = classify_extension("json");
    assert_eq!(kind.write_strategy, WriteStrategy::SidecarOnly);
}

#[test]
fn sse_parser_ignores_reasoning_and_collects_content() {
    let raw = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"你\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"忽略\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"好\"}}]}\n\n",
        "data: [DONE]\n\n",
    );
    let output = collect_sse_content(raw.as_bytes()).expect("parse");
    assert_eq!(output, "你好");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test commenter::scanner::tests::scanner_classifies_json_as_sidecar_only commenter::sse::tests::sse_parser_ignores_reasoning_and_collects_content --manifest-path src-tauri/Cargo.toml`

Expected: FAIL because classification and parser helpers do not exist yet

- [ ] **Step 3: Write minimal implementation**

```rust
// src-tauri/src/commenter/scanner.rs
pub fn classify_extension(ext: &str) -> FileKind {
    match ext {
        "json" => FileKind::sidecar_only("json"),
        "go" | "java" | "py" | "ts" | "js" | "vue" | "sh" | "yaml" | "yml" | "xml" | "properties" | "tpl" => {
            FileKind::annotate_in_place(ext)
        }
        _ => FileKind::skip(ext),
    }
}
```

```rust
// src-tauri/src/commenter/sse.rs
use serde_json::Value;
use std::io::{BufRead, BufReader, Read};

pub fn collect_sse_content(reader: impl Read) -> Result<String, String> {
    let mut content = String::new();
    for line in BufReader::new(reader).lines() {
        let line = line.map_err(|err| err.to_string())?;
        let Some(payload) = line.strip_prefix("data: ") else { continue };
        if payload == "[DONE]" {
            break;
        }
        let value: Value = serde_json::from_str(payload).map_err(|err| err.to_string())?;
        if let Some(delta) = value["choices"][0]["delta"]["content"].as_str() {
            content.push_str(delta);
        }
    }
    Ok(content)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test commenter::scanner::tests::scanner_classifies_json_as_sidecar_only commenter::sse::tests::sse_parser_ignores_reasoning_and_collects_content --manifest-path src-tauri/Cargo.toml`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commenter/scanner.rs src-tauri/src/commenter/prompt.rs src-tauri/src/commenter/http.rs src-tauri/src/commenter/sse.rs src-tauri/src/commenter/models.rs
git commit -m "feat(commenter): add scanner prompt and sse pipeline"
```

## Task 4: Implement Validation, Writeback, Review, And Rollback

**Files:**
- Create: `src-tauri/src/commenter/validate.rs`
- Create: `src-tauri/src/commenter/rollback.rs`
- Modify: `src-tauri/src/commenter/artifacts.rs`
- Modify: `src-tauri/src/commenter/db.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn strong_validation_rejects_markdown_fence() {
    let result = validate_candidate(
        FileValidationInput::source("main.go", "package main\nfunc main() {}\n"),
        "```go\npackage main\nfunc main() {}\n```",
    );
    assert!(matches!(result.decision, ValidationDecision::Reject(_)));
}

#[test]
fn rollback_refuses_when_current_hash_drifted() {
    let outcome = can_overwrite_for_rollback("before-hash", "after-hash", "user-edited-hash");
    assert_eq!(outcome, RollbackGuard::Conflict);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test commenter::validate::tests::strong_validation_rejects_markdown_fence commenter::rollback::tests::rollback_refuses_when_current_hash_drifted --manifest-path src-tauri/Cargo.toml`

Expected: FAIL because validation and rollback guards do not exist yet

- [ ] **Step 3: Write minimal implementation**

```rust
// src-tauri/src/commenter/validate.rs
pub fn validate_candidate(input: FileValidationInput, candidate: &str) -> ValidationResult {
    if candidate.trim().is_empty() {
        return ValidationResult::reject("empty candidate");
    }
    if candidate.contains("```") {
        return ValidationResult::reject("markdown fence");
    }
    if input.extension == "go" && !candidate.contains("package ") {
        return ValidationResult::reject("missing package header");
    }
    ValidationResult::accept()
}
```

```rust
// src-tauri/src/commenter/rollback.rs
pub fn can_overwrite_for_rollback(
    original_hash: &str,
    written_hash: &str,
    current_hash: &str,
) -> RollbackGuard {
    if current_hash == written_hash {
        RollbackGuard::Safe
    } else if current_hash == original_hash {
        RollbackGuard::AlreadyOriginal
    } else {
        RollbackGuard::Conflict
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test commenter::validate::tests::strong_validation_rejects_markdown_fence commenter::rollback::tests::rollback_refuses_when_current_hash_drifted --manifest-path src-tauri/Cargo.toml`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commenter/validate.rs src-tauri/src/commenter/rollback.rs src-tauri/src/commenter/artifacts.rs src-tauri/src/commenter/db.rs
git commit -m "feat(commenter): add validation review and rollback guards"
```

## Task 5: Implement Scheduler, Worker Pool, Retry, Pause, And Recovery

**Files:**
- Create: `src-tauri/src/commenter/scheduler.rs`
- Modify: `src-tauri/src/commenter/db.rs`
- Modify: `src-tauri/src/commenter/models.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn stops_after_success_limit() {
    let mut scheduler = SchedulerHarness::new().with_success_limit(2);
    scheduler.seed_jobs(["a.go", "b.go", "c.go"]);
    scheduler.run().await;
    assert_eq!(scheduler.completed_count(), 2);
    assert_eq!(scheduler.run_status(), RunStatus::StoppedByLimit);
}

#[tokio::test]
async fn restart_recovers_running_run_to_paused() {
    let state = recover_run_status(RunStatus::Running, vec![JobStatus::Leased, JobStatus::Writing]);
    assert_eq!(state.run_status, RunStatus::Paused);
    assert_eq!(state.job_statuses, vec![JobStatus::Pending, JobStatus::ReviewNeeded]);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test commenter::scheduler::tests::stops_after_success_limit commenter::scheduler::tests::restart_recovers_running_run_to_paused --manifest-path src-tauri/Cargo.toml`

Expected: FAIL because scheduler and recovery helpers do not exist yet

- [ ] **Step 3: Write minimal implementation**

```rust
// src-tauri/src/commenter/scheduler.rs
pub struct SchedulerConfig {
    pub global_max_workers: usize,
    pub run_max_workers: usize,
    pub success_limit: Option<u64>,
    pub max_retries: u32,
}

pub fn recover_run_status(run_status: RunStatus, job_statuses: Vec<JobStatus>) -> RecoverySnapshot {
    if matches!(run_status, RunStatus::Running | RunStatus::Scanning | RunStatus::Pausing) {
        let mapped = job_statuses
            .into_iter()
            .map(|status| match status {
                JobStatus::Writing => JobStatus::ReviewNeeded,
                JobStatus::Leased | JobStatus::Requesting | JobStatus::Validating => JobStatus::Pending,
                other => other,
            })
            .collect();
        return RecoverySnapshot {
            run_status: RunStatus::Paused,
            job_statuses: mapped,
        };
    }

    RecoverySnapshot { run_status, job_statuses }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test commenter::scheduler::tests::stops_after_success_limit commenter::scheduler::tests::restart_recovers_running_run_to_paused --manifest-path src-tauri/Cargo.toml`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commenter/scheduler.rs src-tauri/src/commenter/db.rs src-tauri/src/commenter/models.rs src-tauri/src/main.rs
git commit -m "feat(commenter): add worker scheduling and recovery"
```

## Task 6: Expose Tauri Commands, Events, And Frontend Invoke Wrappers

**Files:**
- Create: `src-tauri/src/commenter/events.rs`
- Create: `src-tauri/src/commenter/commands.rs`
- Modify: `src-tauri/src/main.rs`
- Create: `src/lib/commenterTypes.ts`
- Modify: `src/lib/tauri.ts`

- [ ] **Step 1: Write the failing frontend wrapper test**

```js
import assert from 'node:assert/strict';

import { commenterApi } from './tauri.ts';

assert.equal(typeof commenterApi.enqueueRun, 'function');
assert.equal(typeof commenterApi.pauseRun, 'function');
assert.equal(typeof commenterApi.rollbackRun, 'function');

console.log('commenterApi shape PASSED');
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node src/lib/commenterApiShape.test.mjs`

Expected: FAIL with missing `commenterApi`

- [ ] **Step 3: Write minimal implementation**

```ts
// src/lib/commenterTypes.ts
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
```

```ts
// src/lib/tauri.ts additions
export const commenterApi = {
  enqueueRun: (request: CommenterEnqueueRunRequest) => invoke<CommenterRunHandle>('commenter_enqueue_run', { request }),
  startRun: (runId: string) => invoke<void>('commenter_start_run', { runId }),
  pauseRun: (runId: string) => invoke<void>('commenter_pause_run', { runId }),
  resumeRun: (runId: string) => invoke<void>('commenter_resume_run', { runId }),
  cancelRun: (runId: string) => invoke<void>('commenter_cancel_run', { runId }),
  openExternalDiff: (jobId: string) => invoke<void>('commenter_open_external_diff', { jobId }),
  rollbackRun: (runId: string) => invoke<void>('commenter_rollback_run', { runId }),
};
```

- [ ] **Step 4: Run verification**

Run: `pnpm check`

Expected: PASS with the new types and invoke wrappers wired correctly

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commenter/events.rs src-tauri/src/commenter/commands.rs src-tauri/src/main.rs src/lib/commenterTypes.ts src/lib/tauri.ts
git commit -m "feat(commenter): expose commands events and typed wrappers"
```

## Task 7: Add Route, Store, And Tool Entry Shell

**Files:**
- Create: `src/lib/commenterStore.ts`
- Create: `src/lib/commenterView.ts`
- Create: `src/pages/CommentOrchestratorPage.vue`
- Modify: `src/router/index.ts`
- Modify: `src/components/Sidebar.vue`
- Modify: `src/pages/ToolsHubPage.vue`
- Modify: `src/locales/messages.ts`

- [ ] **Step 1: Write the failing test**

```js
import assert from 'node:assert/strict';
import router from '../router/index.ts';

const route = router.getRoutes().find((item) => item.path === '/tools/comment-orchestrator');
assert.ok(route, 'comment orchestrator route should exist');

console.log('comment orchestrator route PASSED');
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node src/lib/commenterRoute.test.mjs`

Expected: FAIL because `/tools/comment-orchestrator` is not registered yet

- [ ] **Step 3: Write minimal implementation**

```ts
// src/router/index.ts addition
{
  path: '/tools/comment-orchestrator',
  component: () => import('../pages/CommentOrchestratorPage.vue'),
}
```

```ts
// src/lib/commenterStore.ts
import { reactive } from 'vue';

export const commenterStore = reactive({
  profiles: [],
  runs: [],
  selectedRunId: null as string | null,
  reviewJobs: [],
  historyRuns: [],
  loading: false,
});
```

- [ ] **Step 4: Run verification**

Run: `pnpm check`

Expected: PASS with route, page import, and store types resolving

- [ ] **Step 5: Commit**

```bash
git add src/lib/commenterStore.ts src/lib/commenterView.ts src/pages/CommentOrchestratorPage.vue src/router/index.ts src/components/Sidebar.vue src/pages/ToolsHubPage.vue src/locales/messages.ts
git commit -m "feat(commenter): add route store and tool shell"
```

## Task 8: Build Project Profile, Queue, Review, History, And Diff UI

**Files:**
- Create: `src/components/commenter/ProjectProfilesPanel.vue`
- Create: `src/components/commenter/QueueRunsTable.vue`
- Create: `src/components/commenter/RunDetailPanel.vue`
- Create: `src/components/commenter/ReviewJobsPanel.vue`
- Create: `src/components/commenter/RunHistoryPanel.vue`
- Create: `src/components/commenter/DiffToolSettingsPanel.vue`
- Modify: `src/pages/CommentOrchestratorPage.vue`

- [ ] **Step 1: Write the failing verification target**

```text
Goal: the page renders all five primary surfaces:
1. project profile form
2. diff tool settings panel
3. queue runs table
4. run detail panel
5. review jobs panel
6. run history panel
```

- [ ] **Step 2: Verify the page does not yet render those panels**

Run: `rg -n "ProjectProfilesPanel|DiffToolSettingsPanel|QueueRunsTable|RunDetailPanel|ReviewJobsPanel|RunHistoryPanel" src/pages/CommentOrchestratorPage.vue`

Expected: no matches before wiring the page

- [ ] **Step 3: Implement the page composition**

```vue
<template>
  <div class="h-full bg-slate-50 p-6 flex flex-col gap-6">
    <header class="flex items-center justify-between gap-4">
      <div>
        <h1 class="text-2xl font-bold text-slate-800">{{ $t('commenter.title') }}</h1>
        <p class="text-sm text-slate-500">{{ $t('commenter.subtitle') }}</p>
      </div>
    </header>

    <div class="grid grid-cols-1 xl:grid-cols-[420px_minmax(0,1fr)] gap-6 min-h-0 flex-1">
      <div class="min-h-0 flex flex-col gap-6">
        <ProjectProfilesPanel />
        <DiffToolSettingsPanel />
      </div>
      <div class="min-h-0 flex flex-col gap-6">
        <QueueRunsTable />
        <RunDetailPanel />
        <ReviewJobsPanel />
        <RunHistoryPanel />
      </div>
    </div>
  </div>
</template>
```

- [ ] **Step 4: Run lint and type checks**

Run: `pnpm lint`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/components/commenter/ProjectProfilesPanel.vue src/components/commenter/QueueRunsTable.vue src/components/commenter/RunDetailPanel.vue src/components/commenter/ReviewJobsPanel.vue src/components/commenter/RunHistoryPanel.vue src/components/commenter/DiffToolSettingsPanel.vue src/pages/CommentOrchestratorPage.vue
git commit -m "feat(commenter): add orchestrator management ui"
```

## Task 9: Run Full Verification And Manual Workflow Checks

**Files:**
- Verify only: `src-tauri/src/commenter/**`, `src/lib/commenterTypes.ts`, `src/lib/commenterStore.ts`, `src/lib/commenterView.ts`, `src/lib/tauri.ts`, `src/pages/CommentOrchestratorPage.vue`, `src/components/commenter/**`, `src/router/index.ts`, `src/components/Sidebar.vue`, `src/pages/ToolsHubPage.vue`, `src/locales/messages.ts`

- [ ] **Step 1: Run focused Rust tests**

Run: `cargo test commenter:: --manifest-path src-tauri/Cargo.toml`

Expected: PASS

- [ ] **Step 2: Run frontend type check**

Run: `pnpm check`

Expected: PASS

- [ ] **Step 3: Run frontend lint**

Run: `pnpm lint`

Expected: PASS

- [ ] **Step 4: Manual verification checklist**

```text
1. Open `/tools/comment-orchestrator`.
2. Create two different project profiles.
3. Configure one run in `auto` and one run in `review`.
4. Set `max files` to a small value and confirm the run stops in `stopped_by_limit`.
5. Set retries to 1, inject a bad token, and confirm jobs fail after retry and the run reports issues.
6. Restore a good token, requeue failed jobs, and confirm progress resumes.
7. Confirm `.json` inputs create sidecar outputs without overwriting the source file.
8. Open a `review_needed` item in the external diff tool.
9. Accept one review item and reject one review item.
10. Restart the app mid-run and confirm the run is restored in `paused` state.
11. Complete a run, then execute rollback and confirm safe files revert while edited files surface conflicts.
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commenter src/lib/commenterTypes.ts src/lib/commenterStore.ts src/lib/commenterView.ts src/lib/tauri.ts src/pages/CommentOrchestratorPage.vue src/components/commenter src/router/index.ts src/components/Sidebar.vue src/pages/ToolsHubPage.vue src/locales/messages.ts
git commit -m "feat(commenter): complete ai comment orchestrator"
```
