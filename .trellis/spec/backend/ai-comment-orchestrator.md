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
}

pub async fn call_chat_completions(request: ChatCompletionsRequest) -> Result<String, String>;

pub async fn call_chat_completions_with_observer<F>(
    request: ChatCompletionsRequest,
    observer: F,
) -> Result<String, String>
where
    F: FnMut(&str);
```

### 3. Contracts

- `CommentAppSettings.api_bearer_token` is the only active runtime credential. It is global and applies to every project profile.
- Frontend project-profile DTOs must not expose or send credential key names or project-scoped bearer tokens. `CommentProjectSettings.credential_profile_key` and `api_bearer_token` may remain serde-defaulted for legacy JSON, but runtime credential resolution ignores them.
- The desktop app does not read API tokens from environment variables for commenter runs. A blank global `api_bearer_token` is a configuration error.
- Missing or empty credentials are job failures. Production code must not fabricate candidates, prepend banners, or otherwise pretend that an AI response exists.
- The HTTP client posts to `{base_url}/v2/chat/completions`, sends a Bearer token, and requests streaming chat-completions output.
- SSE handling must append only `choices[0].delta.content`. `reasoning_content` is ignored and must never be written to source files or candidates.
- Callers that need live UI feedback must use `call_chat_completions_with_observer`; the observer receives every accepted `delta.content` chunk before the final concatenated response is returned.
- The model output must be sanitized and validated before auto-write. Markdown fences, explanatory prefixes, empty output, severe shrink/expansion, and language-structure anomalies go to failure/review instead of direct write.

### 4. Validation & Error Matrix

| Case | Expected Behavior |
| --- | --- |
| Blank global `CommentAppSettings.api_bearer_token` | Job status becomes `failed`; source file remains unchanged |
| Project profile contains legacy `credential_profile_key` or project `api_bearer_token` | Run ignores those fields and still uses the global app token |
| SSE stream includes only `reasoning_content` | Candidate is empty and the job fails |
| SSE stream includes `delta.content` chunks | Chunks are concatenated in order and passed to validation |
| SSE stream emits `delta.content` chunks during a run | `request_started`, `stream_chunk`, and `model_response_completed` events are emitted for the active run/job |
| Candidate contains ````` | Validation rejects and the job enters review/failure handling |
| Review mode with valid candidate | Candidate artifact is written and job becomes `review_needed` |
| Auto mode with valid candidate | Before/candidate artifacts are written, source is overwritten, and rollback hashes are recorded |

### 5. Good/Base/Bad Cases

- Good: global `api_bearer_token = "Bearer ..."` is saved in app settings. Every project run uses the same token and writes only model `delta.content`.
- Base: tests call `update_app_settings` with `api_bearer_token = "test-token"` before starting a local SSE-backed run.
- Bad: no global token is configured. The job must fail with a credential error; it must not create a synthetic annotated candidate or read an environment variable.

### 6. Tests Required

- `cargo test commenter::config::tests::resolves_global_api_bearer_token --manifest-path src-tauri/Cargo.toml`
- `cargo test commenter::config::tests::rejects_missing_global_api_token_without_env_fallback --manifest-path src-tauri/Cargo.toml`
- `cargo test commenter::commands::tests::missing_credential_fails_job_instead_of_building_stub_candidate --manifest-path src-tauri/Cargo.toml`
- `cargo test commenter::commands::tests::enqueue_and_start_run_generates_review_jobs_and_sidecars --manifest-path src-tauri/Cargo.toml`
- `cargo test commenter::commands::tests::accept_review_then_rollback_restores_written_file --manifest-path src-tauri/Cargo.toml`
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

## Scenario: Runtime State And Worker Execution

### 1. Scope / Trigger

- Use this when changing `CommenterCommandSurface`, runtime persistence, run startup, pause/resume/cancel behavior, or worker limits.
- This scenario covers the transition away from prototype-only JSON state toward the structured SQLite state contract.

### 2. Signatures

```rust
pub fn open_app_database(data_root: &Path) -> rusqlite::Result<Connection>;

impl CommenterCommandSurface {
    pub fn new(data_root: impl Into<PathBuf>) -> Self;
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
- `process_run` leases jobs without holding the mutex across file I/O or HTTP awaits.
- Worker capacity is `min(run.max_workers, app_settings.global_max_workers, app_settings.api_concurrency_limit)`, with every value clamped to at least `1`.
- The success/max-file limit prevents dispatching more new jobs after the limit is reached. Already-dispatched jobs may finish naturally.
- Pause/cancel stops dispatching new jobs. In-flight HTTP jobs finish naturally until a cancellation-token implementation is added.
- `delete_run` removes queued, paused, finished, cancelled, stopped, failed, or rolled-back runs from the JSON state and best-effort removes their artifact directory. It must reject `running` and `pausing` runs so active workers are not orphaned.
- `process_run` emits Tauri events on `commenter://state` for run start/resume, file leasing, request start, stream chunks, model completion, job outcome, and run completion. The JSON state stores these events by the time each job is applied.
- Windows builds use `.cargo/config.toml` with `getrandom_backend="windows_legacy"` so test and app binaries avoid loader failures on environments where newer random APIs or old comctl32 manifests are not available.

### 4. Validation & Error Matrix

| Case | Expected Behavior |
| --- | --- |
| New command surface with empty data root | `<data_root>/app.db` exists and `commenter_schema_meta.version = 8` |
| App settings are persisted | `commenter_app_settings.api_bearer_token` exists with default `''` and round-trips saved global tokens |
| Two HTTP jobs and `max_workers = 2` with global/api limits also `2` | Requests overlap; observed peak in-flight HTTP jobs is at least `2` |
| `max_workers = 4`, global limit `2`, api limit `1` | Effective worker capacity is `1` |
| Run enters `pausing` while jobs are active | No new jobs are leased; active jobs finish; terminal state becomes paused or issue-bearing depending on completed outcomes |
| Queued run is deleted | `list_runs` no longer includes it and `get_run_detail` returns `None` |
| Running/pausing run is deleted | Command returns an error asking the caller to cancel or pause first |
| Tauri desktop shell is compiled for `cargo test --lib` | Unit-test binary excludes Tauri command wrappers and dialog shell imports so core tests do not require comctl32 v6 activation context |

### 5. Good/Base/Bad Cases

- Good: a run with two source files and two allowed workers sends two SSE requests concurrently.
- Base: a fresh desktop launch creates both `commenter-state.json` as the transitional snapshot and `app.db` as the structured store.
- Bad: `db.rs` only has unit tests and is never opened by runtime commands.

### 6. Tests Required

- `cargo test commenter::commands::tests::command_surface_initializes_sqlite_app_database --manifest-path src-tauri/Cargo.toml`
- `cargo test commenter::db::tests::app_settings_round_trip_as_global_limits_and_token_row --manifest-path src-tauri/Cargo.toml`
- `cargo test commenter::commands::tests::start_run_uses_configured_workers_for_http_jobs --manifest-path src-tauri/Cargo.toml`
- `cargo test commenter::commands::tests::queued_run_can_be_deleted_before_start --manifest-path src-tauri/Cargo.toml`
- `cargo test commenter::commands::tests::start_run_records_request_and_stream_events --manifest-path src-tauri/Cargo.toml`
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
