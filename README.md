# AI Comment Orchestrator

`ai-comment-orchestrator` is a Windows desktop application built with `Tauri 2`, `Vue 3`, and `Rust`. It scans project files in batches, sends them to an OpenAI-compatible Chat Completions streaming endpoint, generates Chinese comment candidates, and supports review, diff, logging, and rollback workflows.

## What It Does

- Queue runs for one or more project profiles
- Stream AI output over SSE
- Support both `auto` and `review` run modes
- Persist `before`, `candidate`, `sidecar`, request, and response artifacts for each run
- Pause, resume, cancel, and retry runs
- Open external diff tools for human review
- Roll back changes at run level with conflict protection
- Manage global API credentials, concurrency limits, and diff command templates

## Current Workspace Layout

The app is currently organized around three primary workspaces:

- `/settings`: project profile setup and entry settings
- `/workspace`: active runs, logs, streamed file content, and run details
- `/global`: global credentials, concurrency, diff tool, and storage-related settings

## Tech Stack

- Frontend: Vue 3, TypeScript, Vite, Vue Router
- Desktop shell: Tauri 2
- Backend: Rust, Tokio, reqwest, rusqlite, serde
- Package manager: pnpm

## Quick Start

### Install dependencies

```powershell
pnpm install
```

### Configure credentials

The recommended local setup is an environment variable:

```powershell
$env:OPENAI_API_KEY = "your-token"
```

Credential resolution rules in the backend:

- If a project profile contains `API Bearer Token`, that value wins
- Otherwise `credential_profile_key` is treated as an environment variable name
- Real tokens should never be hardcoded in source, docs, or committed sample data

Verified Tencent endpoint example:

```text
https://unvcoding.copilot.qq.com
```

### Start the app in development

```powershell
pnpm tauri:dev
```

## Common Commands

```powershell
pnpm install
pnpm lint
pnpm check
cargo test --manifest-path src-tauri/Cargo.toml
pnpm tauri:dev
pnpm tauri:exe
```

Command notes:

- `pnpm check`: frontend typecheck plus smoke tests
- `cargo test --manifest-path src-tauri/Cargo.toml`: Rust unit and integration tests
- `pnpm tauri:exe`: build a double-clickable native Windows executable

## Windows Packaging

### Build a standalone EXE

```powershell
pnpm tauri:exe
```

This runs the frontend production build and then produces the Tauri release binary.

Typical output path:

```text
src-tauri/target/release/ai-comment-orchestrator.exe
```

If `CARGO_TARGET_DIR` is set in the environment, Cargo writes artifacts there instead. Example:

```text
D:\Rust\target\release\ai-comment-orchestrator.exe
```

### Build an installer

```powershell
pnpm tauri build
```

The current `tauri.conf.json` bundle target is `nsis`, so installer artifacts are expected under a path like:

```text
src-tauri/target/release/bundle/nsis/
```

Most Windows machines also need Microsoft Edge WebView2 Runtime installed to run the app.

## Runtime Data

The application stores runtime data under the Tauri app data directory. Important files and directories include:

- `app.db`: SQLite database
- `commenter-state.json`: transitional JSON state snapshot
- `commenter/runs/<run_key>/`: per-run artifact directory

Run artifacts commonly include:

- `before/`
- `candidates/`
- `sidecars/`
- `request/`
- `response/`
- related execution logs and events

## Repository Structure

- `src/`: Vue pages, components, frontend state, and Tauri invoke wrappers
- `src-tauri/src/commenter/`: Rust backend domain modules
- `src-tauri/tests/`: Rust integration tests
- `.trellis/spec/`: project-specific specs, contracts, and thinking guides
- `docs/superpowers/`: design notes and implementation plans

## Verification

Recommended validation before handoff or commit:

```powershell
pnpm check
cargo test --manifest-path src-tauri/Cargo.toml
```

For packaging-related changes, also run:

```powershell
pnpm tauri:exe
```

## Notes

- SQLite is initialized at runtime, but some state still exists in a transitional JSON persistence layer
- Pause or cancel stops new work from being dispatched, but in-flight HTTP requests finish naturally
- Validation and review are intentionally conservative to avoid writing risky AI output directly into source files
