# AIUse-AddComment Agent Notes

## Project Overview

This repository builds a Tauri 2 desktop application named `ai-comment-orchestrator`.
Its main purpose is to batch-scan project files, call a chat-completions SSE API, generate Chinese comment candidates, stage risky output for review, and support rollback.

The current stack is:

- Frontend: Vue 3, TypeScript, Vite, Vue Router
- Desktop shell: Tauri 2
- Backend: Rust, Tokio, reqwest, rusqlite, serde
- Package manager: pnpm

## Important Directories

- `src/`: Vue UI, typed Tauri invoke wrappers, mock backend, smoke tests.
- `src-tauri/src/commenter/`: Rust commenter domain modules.
- `src-tauri/tests/`: Rust integration tests for Tauri/commenter behavior.
- `.trellis/spec/`: project-specific implementation contracts and thinking guides.
- `docs/superpowers/`: design and implementation planning notes.

## Commenter Backend Shape

The Rust commenter module is split by responsibility:

- `models.rs`: run/job/settings/event DTOs and state enums.
- `config.rs`: credential resolution. Direct `api_bearer_token` wins; otherwise `credential_profile_key` is treated as an environment variable name.
- `http.rs`: POST `/v2/chat/completions` and collect streaming `choices[0].delta.content`.
- `events.rs`: Tauri event payloads on `commenter://state`, including request lifecycle and streamed AI chunks.
- `prompt.rs`: Chinese annotation prompt construction.
- `scanner.rs`: project scanning and built-in ignored directories.
- `validate.rs`: candidate safety checks.
- `artifacts.rs`: run artifact directory layout.
- `rollback.rs`: hash-based rollback guard.
- `commands.rs`: Tauri command surface, JSON transitional state, runtime SQLite initialization, concurrent run execution.
- `.cargo/config.toml`: Windows builds force getrandom's legacy backend to avoid `ProcessPrng` loader failures in older/compatibility environments.
- `db.rs`: SQLite schema and migrations. Runtime now initializes `<data_root>/app.db`; JSON snapshot persistence still exists during migration.

## Runtime Data

The app stores commenter data under the configured Tauri app data directory. In tests this is usually a temporary `.commenter-data` directory.

Important runtime files:

- `app.db`: SQLite schema, initialized on command-surface startup.
- `commenter-state.json`: transitional JSON state snapshot.
- `commenter/runs/<run_key>/`: before snapshots, candidates, sidecars, request/response/log folders.

Runs can be deleted from the queue when they are not `running` or `pausing`. Deletion removes the run from `commenter-state.json` and best-effort removes its artifact directory.

The UI subscribes to `commenter://state` and shows live execution logs. Stream chunks are emitted as `stream_chunk` events while HTTP SSE is still arriving; final state polling remains as a backup.

## Credential Rules

Do not hardcode real tokens in source files, docs, tests, or committed sample data.

Recommended local setup:

```powershell
$env:OPENAI_API_KEY = "your-token"
pnpm tauri:dev
```

For the verified Tencent endpoint, configure the profile API base URL as:

```text
https://unvcoding.copilot.qq.com
```

Missing credentials must fail jobs honestly. The code must not generate fake banner candidates.

## Development Commands

Install dependencies:

```powershell
pnpm install
```

Run frontend typecheck and smoke tests:

```powershell
pnpm check
```

Run Rust tests:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

Run the app in development:

```powershell
pnpm tauri:dev
```

## Build A Double-Clickable Windows EXE

Build the standalone executable:

```powershell
pnpm install
pnpm tauri:exe
```

The double-clickable exe is generated at:

```text
src-tauri/target/release/ai-comment-orchestrator.exe
```

Because `.cargo/config.toml` sets Windows rustflags, use the repo root as the working directory when building so Cargo sees the compatibility configuration.

This command uses `tauri build --no-bundle`, so it produces the raw app executable. For an installer, run:

```powershell
pnpm tauri build
```

The NSIS installer appears under:

```text
src-tauri/target/release/bundle/nsis/
```

Windows machines need Microsoft Edge WebView2 Runtime installed, which is normally already present on modern Windows.

## Verification Before Handoff

Before reporting work as complete, run:

```powershell
pnpm check
cargo test --manifest-path src-tauri/Cargo.toml
```

For packaging changes, also run:

```powershell
pnpm tauri:exe
```

## Known Remaining Debt

- SQLite is initialized at runtime, but the command surface still uses `commenter-state.json` as the transitional source of truth. A future migration should move profiles, runs, jobs, events, and artifacts into SQLite CRUD helpers.
- Pause/cancel stops dispatching new work, but in-flight HTTP requests finish naturally. A future cancellation-token pass should abort active requests.
- Review and validation are intentionally conservative but not yet a full semantic diff engine.

## Agent Notes

- Read `.trellis/spec/backend/ai-comment-orchestrator.md` before changing the commenter backend.
- Keep generated model output out of source files unless it passed validation or was explicitly accepted in review mode.
- Use tests with local SSE servers for HTTP/SSE behavior; avoid real network calls in automated tests.
- Search before changing shared settings or enums because Rust DTOs and TypeScript types must stay aligned.
