# AI Comment Orchestrator

> Contracts for the Rust commenter backend that turns queued files into AI-generated Chinese annotation candidates.

## Scenario: Chat Completions Execution

### 1. Scope / Trigger

- Use this when changing `src-tauri/src/commenter/http.rs`, `prompt.rs`, `config.rs`, `commands.rs`, or any command that starts/retries a commenter run.
- The feature crosses frontend settings, Rust command state, local files, HTTP/SSE, validation, artifacts, and rollback state.

### 2. Signatures

```rust
pub fn resolve_bearer_token(app_settings: &CommentAppSettings) -> Result<String, ConfigError>;

pub fn describe_bearer_token_source(app_settings: &CommentAppSettings) -> Result<String, ConfigError>;

pub struct ChatCompletionsRequest {
    pub base_url: String,
    pub bearer_token: String,
    pub model: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub timeout_secs: u64,
    pub context: ChatCompletionsRequestContext,
}

pub async fn call_chat_completions(request: ChatCompletionsRequest) -> Result<String, String>;

pub async fn call_chat_completions_with_observer<F>(
    request: ChatCompletionsRequest,
    observer: F,
) -> Result<String, String>
where
    F: FnMut(&str);

pub const DEFAULT_STREAM_IDLE_TIMEOUT_SECS: u64 = 60;
pub const DEFAULT_API_MODEL: &str = "glm-5.1";

pub fn is_sse_done_line(line: &str) -> bool;
```

### 3. Contracts

- `CommentAppSettings.api_bearer_token` is the only active runtime credential. It is global and applies to every project profile.
- Frontend project-profile DTOs must not expose or send credential key names or project-scoped bearer tokens. `CommentProjectSettings.credential_profile_key` and `api_bearer_token` may remain serde-defaulted for legacy JSON, but runtime credential resolution ignores them.
- The desktop app does not read API tokens from environment variables for commenter runs. A blank global `api_bearer_token` is a configuration error.
- Missing or empty credentials are job failures. Production code must not fabricate candidates, prepend banners, or otherwise pretend that an AI response exists.
- The HTTP client posts to `{base_url}/v2/chat/completions`, sends a Bearer token, and requests streaming chat-completions output.
- Windows builds use reqwest's `rustls-tls-native-roots` feature so HTTPS validation includes certificates trusted by the Windows certificate store, including an explicitly trusted enterprise or Fiddler capture root. Certificate validation must remain enabled; production code must not use `danger_accept_invalid_certs(true)`.
- Blank or newly created project model settings resolve to `glm-5.1`; existing non-blank custom model names remain unchanged.
- SSE handling must append only `choices[0].delta.content`. `reasoning_content` is ignored and must never be written to source files or candidates.
- `data: [DONE]` is terminal even when the upstream keeps the HTTP connection open.
- Waiting for the next response-body chunk is bounded by `min(max(request.timeout_secs, 30), DEFAULT_STREAM_IDLE_TIMEOUT_SECS)`. No bytes for that interval produces `sse stream idle timeout`; the request timeout remains the total bound.
- Callers that need live UI feedback must use `call_chat_completions_with_observer`; the observer receives every accepted `delta.content` chunk before the final concatenated response is returned.
- Worker `stream_chunk` events coalesce adjacent observer chunks at 512 characters or 100 milliseconds. The first chunk is immediate, buffered content flushes on completion/error, and messages split at 1200 characters without data loss.
- `stream_chunk` is a transient Tauri delivery event: it must not enter `StoredRun.events`, `commenter-state.json`, or `commenter_get_run_detail`. Full response/candidate text remains available in managed artifacts.
- The model output must be sanitized and validated before auto-write. Markdown fences, explanatory prefixes, empty output, severe shrink/expansion, and language-structure anomalies go to failure/review instead of direct write.

### 4. Validation & Error Matrix

| Case | Expected Behavior |
| --- | --- |
| Blank global `CommentAppSettings.api_bearer_token` | Job status becomes `failed`; source file remains unchanged |
| HTTPS interception root is trusted by Windows | TLS validation succeeds through the intercepting proxy; request and response bodies can be captured |
| HTTPS interception root is absent or untrusted | TLS validation fails explicitly; the client does not bypass certificate verification |
| Project profile contains legacy `credential_profile_key` or project `api_bearer_token` | Run ignores those fields and still uses the global app token |
| SSE stream includes only `reasoning_content` | Candidate is empty and the job fails |
| SSE stream includes `delta.content` chunks | Chunks are concatenated in order and passed to validation |
| SSE stream emits `delta.content` chunks during a run | `request_started`, transient `stream_chunk`, and `model_response_completed` events are emitted; only structural events are retained |
| SSE emits `[DONE]` but keeps the connection open | Reader returns accumulated content immediately; the job does not remain `requesting` |
| SSE emits no bytes for 60 seconds | Request fails with `sse stream idle timeout`; retry/failure emits `job_failed` and settles the UI slice |
| Candidate contains ````` | Validation rejects and the job enters review/failure handling |
| Review mode with valid candidate | Candidate artifact is written and job becomes `review_needed` |
| Auto mode with valid candidate | Before/candidate artifacts are written, source is overwritten, and rollback hashes are recorded |

### 5. Good/Base/Bad Cases

- Good: global `api_bearer_token = "Bearer ..."` is saved in app settings. Every project run uses the same token and writes only model `delta.content`.
- Base: tests call `update_app_settings` with `api_bearer_token = "test-token"` before starting a local SSE-backed run; `[DONE]` completes without socket closure. Windows uses its trusted root store for TLS validation.
- Bad: no global token is configured, a connected stream silently stops producing bytes, or certificate validation is globally disabled to make an intercepting proxy work. The job must fail explicitly; it must not fabricate a candidate or weaken TLS verification.

### 6. Tests Required

- `cargo test commenter::config::tests::resolves_global_api_bearer_token --manifest-path src-tauri/Cargo.toml`
- `cargo test commenter::config::tests::rejects_missing_global_api_token_without_env_fallback --manifest-path src-tauri/Cargo.toml`
- `cargo test commenter::commands::tests::missing_credential_fails_job_instead_of_building_stub_candidate --manifest-path src-tauri/Cargo.toml`
- `cargo test commenter::commands::tests::enqueue_and_start_run_generates_review_jobs_and_sidecars --manifest-path src-tauri/Cargo.toml`
- `cargo test commenter::commands::tests::accept_review_then_rollback_restores_written_file --manifest-path src-tauri/Cargo.toml`
- `cargo tree -e features -i reqwest@0.12.28 --manifest-path src-tauri/Cargo.toml` includes `rustls-tls-native-roots` and does not enable `rustls-tls-webpki-roots`
- `cargo test --manifest-path src-tauri/Cargo.toml`

### 7. Wrong vs Correct

#### Wrong

```rust
if app_settings.api_bearer_token.trim().is_empty() {
    return Ok(format!("// 注释：由 AI 批量注释编排器生成。\n{source}"));
}
```

This produces fake success and hides credential/configuration failures.

#### Correct

```rust
let bearer_token = resolve_bearer_token(app_settings)
    .map_err(|error| format!("credential resolution failed: {error:?}"))?;
```

This keeps run state honest: no credential means no model call and no candidate.

#### Wrong: disable TLS certificate validation for capture

```rust
Client::builder().danger_accept_invalid_certs(true).build()?;
```

#### Correct: trust only roots installed in the Windows certificate store

```toml
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls-native-roots"] }
```

This permits an explicitly trusted enterprise/Fiddler root without accepting arbitrary certificates.

## Scenario: CodeBuddy-Compatible HTTP Telemetry

### 1. Scope / Trigger

- Use this when changing the single per-file HTTP request envelope, SSE usage parsing, `/v2/report`, or either source-file write path.
- The contract spans Vue settings, Tauri DTOs, JSON/SQLite compatibility, HTTP/SSE, validation, review acceptance, and rollback artifacts.

### 2. Signatures

```rust
pub struct ChatCompletionsRequest {
    pub base_url: String,
    pub bearer_token: String,
    pub model: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub timeout_secs: u64,
    pub context: ChatCompletionsRequestContext,
}

pub struct ChatUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cached_tokens: u64,
    pub cached_write_tokens: u64,
    pub cached_miss_tokens: u64,
}

pub fn added_line_count(before: &str, candidate: &str) -> usize;
pub fn utf16_len(value: &str) -> usize;
pub fn language_id(relative_path: &str) -> &'static str;
```

```typescript
export interface CommenterRunSettingsView {
  global_max_workers: number;
  api_concurrency_limit: number;
  api_bearer_token: string;
}
```

### 3. Contracts

- There is exactly one runtime request flow: concurrent, per-file, CodeBuddy-compatible whole-file HTTP. No request-mode DTO, UI selector, SQLite column, `current` body, or project-level Agent tool loop is active.
- The request body order is `model`, `max_tokens`, `temperature`, `reasoningEffort`, `reasoning_summary`, `reasoning_effort`, `messages`, `stream`; it contains no `tools` or `tool_choice`.
- A successful request enqueues `chat_request_send`, `chat_message_send`, `chat_message_response`, `chat_message_status`, and `chat_request_response`. Final SSE `usage` supplies token fields.
- A successful real source replacement enqueues one `chat_tool_action` and one `code_edit`. Auto mode reports only after `fs::write` succeeds; review mode reports only after explicit acceptance writes the candidate.
- `code_edit.lineCount` is the inserted-side line count from original to complete candidate, `characterCount` is the complete candidate UTF-16 code-unit length, `fileCount=1`, and `source=agent`.
- Rejection, validation/request failure, and failed writes never emit `code_edit`. The four non-core events `plugin_status`, `workspace_file_change`, `mcp_tool_call`, and `page_show` are not emitted.
- Reporter batches from the first queued event for approximately two seconds. Each `/v2/report` batch creates independent `X-Request-Trace-Id` and `X-Request-ID` values, preserves TLS verification, and logs network or non-2xx failures explicitly without changing the commenter job outcome.
- Legacy JSON fields and an already-existing SQLite `request_mode` column may remain on disk but are ignored. New schemas and DTOs do not create or expose the field.

### 4. Validation & Error Matrix

| Case | Expected Behavior |
| --- | --- |
| HTTP succeeds with final SSE usage | Five chat lifecycle events contain the same conversation/request/message IDs and the parsed usage |
| HTTP, SSE, validation, or source write fails | Job follows existing failure/review behavior; no `code_edit` is queued |
| Auto mode source write succeeds | Real write is followed by `chat_tool_action` and exact diff/UTF-16 `code_edit` |
| Review candidate is rejected | Source remains unchanged and no write telemetry is emitted |
| Review candidate is accepted and written | Write telemetry is emitted only after the successful write |
| `/v2/report` returns non-2xx | Explicit error is logged; batch is not treated as successful |
| Legacy settings contain `request_mode` | Deserialization/SQLite access ignores it; runtime behavior remains the single HTTP flow |

### 5. Good/Base/Bad Cases

- Good: an SSE response ends with usage, validation accepts the candidate, the source write succeeds, and Fiddler observes chat lifecycle events followed by real write events with exact metrics.
- Base: review mode creates artifacts without write telemetry, then acceptance writes and reports once.
- Bad: emit `code_edit` when a candidate is merely generated, synthesize Tab completion adoption, reuse chat request IDs for `/v2/report`, or ignore a report HTTP 500.

### 6. Tests Required

- `commenter::http::tests::http_compatible_body_matches_codebuddy_top_level_order` asserts stable body order and absence of Agent fields.
- `commenter::http::tests::parse_sse_line_captures_final_usage` asserts final token extraction.
- `commenter::telemetry::tests::code_edit_metrics_match_line_diff_and_utf16_contract` asserts inserted-line, UTF-16, and language mapping rules.
- Command tests must keep auto/review writes, failure, acceptance, rejection, and rollback behavior covered.
- `cargo test --manifest-path src-tauri/Cargo.toml`, `pnpm run check`, and `pnpm run lint` must pass.

### 7. Wrong vs Correct

#### Wrong

```rust
telemetry.report_file_write(&context, path, before, candidate);
fs::write(path, candidate)?;
```

This counts a write that may fail and violates the dashboard's real-write semantics.

#### Correct

```rust
fs::write(path, candidate)?;
telemetry.report_file_write(&context, relative_path, before, candidate);
```

The event now describes a source replacement that actually happened.

## Scenario: Runtime State And Worker Execution

### 1. Scope / Trigger

- Use this when changing `CommenterCommandSurface`, runtime persistence, run startup, pause/resume/cancel behavior, or worker limits.
- This scenario covers the transition away from prototype-only JSON state toward the structured SQLite state contract.

### 2. Signatures

```rust
pub fn open_app_database(data_root: &Path) -> rusqlite::Result<Connection>;

impl CommenterCommandSurface {
    pub fn new(data_root: impl Into<PathBuf>) -> Self;
    pub fn delete_project_profile(&self, project_key: &str) -> Result<CommenterProjectProfileView, String>;
    pub fn delete_run(&self, run_key: &str) -> Result<CommenterRunRecord, String>;
    async fn process_run(
        &self,
        app: Option<&AppHandle>,
        run_key: &str,
        from_resume: bool,
    ) -> Result<CommenterRunDetail, String>;
}
```

### 3. Contracts

- Runtime state root contains `app.db` at `<data_root>/app.db`.
- `CommenterCommandSurface::new()` must create the data root, open `app.db`, and run SQLite migrations before serving commands.
- JSON snapshot persistence (`commenter-state.json`) may remain during the transition, but SQLite must not be pure dead code.
- Transitional snapshots use compact `serde_json::to_vec` serialization because the entire state is rewritten at lease/outcome boundaries. Request/response debug artifacts remain pretty JSON.
- `process_run` leases jobs without holding the mutex across file I/O or HTTP awaits.
- Worker capacity is `min(run.max_workers, app_settings.global_max_workers, app_settings.api_concurrency_limit)`, with every value clamped to at least `1`.
- A project profile may be deleted only when no stored run references its `project_key`. Deletion returns the removed profile and persists state; referenced or unknown profiles return an error without mutation.
- The success/max-file limit prevents dispatching more new jobs after the limit is reached. Already-dispatched jobs may finish naturally.
- `run.max_files == 0` means unlimited. The current UI always creates whole-project runs with `0`; positive values remain supported only for legacy/API callers.
- Pause/cancel stops dispatching new jobs. In-flight HTTP jobs finish naturally until a cancellation-token implementation is added.
- On startup, recovery preserves terminal jobs (`done`, `review_needed`, `skipped`, `failed`) and requeues only in-flight request/validation jobs. Resume therefore continues at file granularity; an interrupted file may run again, but completed files do not.
- `delete_run` removes queued, paused, finished, cancelled, stopped, failed, or rolled-back runs from the JSON state and best-effort removes their artifact directory. It must reject `running` and `pausing` runs so active workers are not orphaned.
- `process_run` emits Tauri events on `commenter://state` for run start/resume, file leasing, request start, stream chunks, model completion, job outcome, and run completion. The JSON state stores structural lifecycle events by the time each job is applied, but never stores `stream_chunk` payloads.
- Candidate text is disk-backed after its candidate artifact is written. `StoredJob.candidate_content` is a legacy fallback only, is omitted when empty, and is compacted on load when the artifact exists.
- Loading a legacy JSON snapshot removes retained `stream_chunk` events and artifact-backed candidate duplicates, then rewrites the compact snapshot before serving commands.
- `commenter_get_run_detail` filters `stream_chunk` defensively so legacy or malformed in-memory state cannot amplify polling across Tauri IPC.
- Windows builds use `.cargo/config.toml` with `getrandom_backend="windows_legacy"` so test and app binaries avoid loader failures on environments where newer random APIs or old comctl32 manifests are not available.

### 4. Validation & Error Matrix

| Case | Expected Behavior |
| --- | --- |
| New command surface with empty data root | `<data_root>/app.db` exists and `commenter_schema_meta.version = 9` |
| App settings are persisted | `commenter_app_settings.api_bearer_token` exists; any legacy `request_mode` value is ignored |
| Two HTTP jobs and `max_workers = 2` with global/api limits also `2` | Requests overlap; observed peak in-flight HTTP jobs is at least `2` |
| Delete a profile referenced by any run | Reject deletion and preserve the profile |
| `max_workers = 4`, global limit `2`, api limit `1` | Effective worker capacity is `1` |
| Run enters `pausing` while jobs are active | No new jobs are leased; active jobs finish; terminal state becomes paused or issue-bearing depending on completed outcomes |
| Queued run is deleted | `list_runs` no longer includes it and `get_run_detail` returns `None` |
| Running/pausing run is deleted | Command returns an error asking the caller to cancel or pause first |
| Tauri desktop shell is compiled for `cargo test --lib` | Unit-test binary excludes Tauri command wrappers and dialog shell imports so core tests do not require comctl32 v6 activation context |
| Legacy snapshot contains stream chunks and an artifact-backed candidate duplicate | Startup removes both duplicates and persists a compact snapshot |
| Desktop restarts during a run | Terminal jobs stay terminal; only interrupted jobs return to pending |

### 5. Good/Base/Bad Cases

- Good: a run with two source files and two allowed workers sends two SSE requests concurrently.
- Base: a fresh desktop launch creates both `commenter-state.json` as the transitional snapshot and `app.db` as the structured store.
- Bad: `db.rs` only has unit tests and is never opened by runtime commands.

### 6. Tests Required

- `cargo test commenter::commands::tests::command_surface_initializes_sqlite_app_database --manifest-path src-tauri/Cargo.toml`
- `cargo test commenter::db::tests::app_settings_round_trip_as_global_limits_and_token_row --manifest-path src-tauri/Cargo.toml`
- `cargo test commenter::commands::tests::start_run_uses_configured_workers_for_http_jobs --manifest-path src-tauri/Cargo.toml`
- `cargo test commenter::commands::tests::queued_run_can_be_deleted_before_start --manifest-path src-tauri/Cargo.toml`
- `cargo test commenter::commands::tests::start_run_keeps_stream_chunks_transient --manifest-path src-tauri/Cargo.toml`
- `cargo test commenter::commands::tests::load_state_compacts_legacy_stream_events_and_artifact_backed_candidates --manifest-path src-tauri/Cargo.toml`
- `cargo test commenter::http::tests::done_marker_finishes_without_waiting_for_connection_close --manifest-path src-tauri/Cargo.toml`
- `cargo test commenter::http::tests::idle_stream_returns_explicit_timeout_error --manifest-path src-tauri/Cargo.toml`
- `cargo test commenter::scheduler::tests::restart_requeues_only_inflight_jobs_and_preserves_terminal_jobs --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml`

### 7. Wrong vs Correct

#### Wrong

```rust
for job in jobs {
    process_single_job_async(job).await;
}
```

This serializes all HTTP work and makes the worker settings misleading.

#### Correct

```rust
let mut running_jobs = FuturesUnordered::new();
while running_jobs.len() < worker_limit {
    running_jobs.push(async move {
        let outcome = process_single_job_async(&action, &run_paths).await;
        (action, outcome)
    });
}
```

This lets independent file jobs overlap while keeping state mutation behind short mutex sections.

#### Wrong: retain and poll the live stream history

```rust
emit_commenter_event(app, &payload);
events.push(payload);
```

Persisting a high-volume delivery payload makes every snapshot rewrite and run-detail poll copy the complete response again.

#### Correct: broadcast transient chunks, persist structural events

```rust
emit_commenter_event(app, &payload);
// StreamChunk is intentionally absent from StoredRun.events.
```

The live UI receives the ordered chunk immediately. Restart/history views obtain full text from the response or candidate artifact instead of replaying persisted chunks.

## Scenario: On-Demand Run File Preview

### 1. Scope / Trigger

- Use this when changing run file preview tabs, artifact lookup, or the Tauri commands that expose candidate/original text.

### 2. Signatures

```rust
pub fn get_candidate_text(&self, run_key: &str, relative_path: &str) -> Result<String, String>;
pub fn get_original_text(&self, run_key: &str, relative_path: &str) -> Result<String, String>;
```

```typescript
commenterApi.getCandidateText(run_key: string, relative_path: string): Promise<string>;
commenterApi.getOriginalText(run_key: string, relative_path: string): Promise<string>;
```

### 3. Contracts

- Original lookup first reads `<before_root>/<relative_path>.before`, including a valid empty snapshot.
- If no before-artifact exists yet, original lookup may fall back to the scanned job's stored absolute source path.
- Candidate lookup prefers the candidate artifact and falls back to the sidecar artifact.
- Missing runs/artifacts return an empty string so the UI can show an unavailable state; I/O and path-safety failures return an error.
- The frontend loads original text only when Original or Diff is selected and discards stale async results after the selected file changes.

### 4. Validation & Error Matrix

| Case | Expected behavior |
| --- | --- |
| Before-artifact exists | Return its exact UTF-8 content |
| Empty before-artifact exists | Return empty content; do not substitute the current rewritten source |
| Snapshot absent but job exists | Read the job's source path |
| Run/path is absent | Return `Ok("")` |
| Artifact path escapes its root | Return an error |

### 5. Good/Base/Bad Cases

- Good: Diff shows immutable before-content beside the generated candidate.
- Base: a pending job without a snapshot shows its current source file.
- Bad: Original silently shows the post-write candidate because empty and missing artifacts were treated as the same state.

### 6. Tests Required

- `commenter::commands::tests::get_original_text_returns_before_artifact_content` asserts exact snapshot retrieval.
- `src/lib/commenterApiShape.test.ts` asserts the frontend bridge exposes both text commands.
- `src/lib/commenterStreamPanel.test.ts` asserts Diff and Original have real content branches.

### 7. Wrong vs Correct

#### Wrong

```vue
<button @click="active_tab = 'original'">Original</button>
<!-- no original content branch -->
```

#### Correct

```vue
<section v-else-if="active_tab === 'original'">
  <pre v-if="original_text">{{ original_text }}</pre>
</section>
```
