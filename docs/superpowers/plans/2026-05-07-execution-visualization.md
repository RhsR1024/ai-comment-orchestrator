# Execution Visualization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the opaque "click Start and hope" experience in `CommentOrchestratorPage` with a live execution log, run-level header, lazy-loaded workspace tree, and AI streaming panel — so the user can see at every moment which file is processing, what the model is emitting, and whether each file succeeded or failed.

**Architecture:** Page-level execution-log sidebar + run-header strip live above the existing tab strip; the Detail tab is rewritten as a tree + stream split. Pure derivation functions (`commenterFileLog`, `commenterStreamSlice`) feed the UI; `commenterStore` gains a `live_streams` slice fed by the existing `commenter://state` event channel; two new read-only Tauri commands (`commenter_list_dir`, `commenter_get_candidate_text`) back the tree and the locked-file viewer.

**Tech Stack:** Vue 3 (Composition API, `<script setup>`), TypeScript, Tauri 2 (Rust 1.x, rusqlite), Vite, lucide-vue-next, `tsx + node:assert/strict` for frontend smoke tests, `cargo test` for Rust.

**Spec:** `docs/superpowers/specs/2026-05-07-execution-visualization-design.md`

**Verification commands:**
- Frontend: `pnpm check` (vue-tsc + smoke)
- Backend: `cargo test commenter --manifest-path src-tauri/Cargo.toml`

---

## File Structure

| Path | Action | Responsibility |
| --- | --- | --- |
| `src/lib/commenterFileLog.ts` | Create | Pure: derive `FileLogEntry[]` from `(jobs, events)` |
| `src/lib/commenterFileLog.test.ts` | Create | Pure unit tests for the above |
| `src/lib/commenterStreamSlice.ts` | Create | Pure: reduce events into a `LiveStreamSlice` map with LRU + 5MB cap |
| `src/lib/commenterStreamSlice.test.ts` | Create | Pure unit tests for the reducer |
| `src/lib/commenterStore.ts` | Modify | Add `live_streams` reactive map; call slice reducer in `appendExecutionEvent`; clear on `selectRun` |
| `src/lib/tauri.ts` | Modify | Add `listDir` and `getCandidateText` to `commenterApi` |
| `src/lib/mockCommenterBackend.ts` | Modify | Provide stub responses for the two new APIs (so dev mode keeps working) |
| `src-tauri/src/commenter/commands.rs` | Modify | Add `list_dir` and `get_candidate_text` methods + `#[tauri::command]` wrappers |
| `src-tauri/src/lib.rs` | Modify | Register the two new commands in `tauri::generate_handler![]` |
| `src/components/commenter/RunHeaderStrip.vue` | Create | Run name / status / progress / Pause / Resume / Cancel |
| `src/components/commenter/ExecutionLogPanel.vue` | Rewrite | Compact per-file rows from `buildFileLogEntries`, expandable phase timeline |
| `src/components/commenter/WorkspaceTreePanel.vue` | Create | Lazy-loaded source tree, queue highlight, current-file auto-expand |
| `src/components/commenter/StreamContentPanel.vue` | Create | Live/locked text viewer with badges and auto-fetch fallback |
| `src/components/commenter/RunDetailPanel.vue` | Rewrite | Thin container holding tree + stream + follow-mode |
| `src/pages/CommentOrchestratorPage.vue` | Modify | New runtime workspace layout (header, log column, tab area) |
| `src/lib/commenterRunHeader.test.ts` | Create | Contract test for `RunHeaderStrip.vue` |
| `src/lib/commenterWorkspaceTree.test.ts` | Create | Contract test for `WorkspaceTreePanel.vue` |
| `src/lib/commenterStreamPanel.test.ts` | Create | Contract test for `StreamContentPanel.vue` |
| `src/lib/commenterExecutionLog.test.ts` | Modify | Drop flat-log assertions; assert new compact behavior |
| `package.json` | Modify | Append the four new test scripts to `smoke` |
| `src/locales/messages.ts` | Modify | Add new translation keys (status badges, stream/tree labels, error placeholders) |

---

## Task 1: Pure aggregation function `commenterFileLog`

**Files:**
- Create: `src/lib/commenterFileLog.ts`
- Test: `src/lib/commenterFileLog.test.ts`

- [ ] **Step 1: Write the failing test**

Write `src/lib/commenterFileLog.test.ts`:

```ts
import assert from 'node:assert/strict';

import { buildFileLogEntries } from './commenterFileLog';
import type { CommenterEventPayload, CommenterJobRecord } from './commenterTypes';

function job(relative_path: string, status: CommenterJobRecord['status']): CommenterJobRecord {
  return {
    id: 0,
    relative_path,
    status,
    language_hint: null,
    write_strategy: 'auto',
    retry_count: 0,
    error_message: null,
    before_artifact_path: null,
    candidate_artifact_path: null,
    sidecar_artifact_path: null
  };
}

function event(
  kind: CommenterEventPayload['kind'],
  relative_path: string | null,
  at: number,
  message = ''
): CommenterEventPayload {
  return {
    kind,
    run_key: 'r1',
    relative_path,
    level: kind === 'job_failed' ? 'error' : 'info',
    message,
    created_at: at
  };
}

// 1. Empty inputs return []
assert.deepEqual(buildFileLogEntries([], []), []);

// 2. Single file with full phase sequence keeps phases in order
{
  const jobs = [job('src/a.ts', 'done')];
  const events = [
    event('request_started', 'src/a.ts', 100),
    event('stream_chunk', 'src/a.ts', 110, 'hello '),
    event('stream_chunk', 'src/a.ts', 120, 'world'),
    event('model_response_completed', 'src/a.ts', 130),
    event('job_updated', 'src/a.ts', 140)
  ];
  const result = buildFileLogEntries(jobs, events);
  assert.equal(result.length, 1);
  assert.equal(result[0].relative_path, 'src/a.ts');
  assert.equal(result[0].status, 'done');
  assert.deepEqual(result[0].phases.map((p) => p.phase), [
    'requested',
    'first_chunk',
    'response_done',
    'written'
  ]);
  assert.equal(result[0].started_at, 100);
  assert.equal(result[0].ended_at, 140);
}

// 3. Interleaved events for multiple files group correctly
{
  const jobs = [job('src/a.ts', 'done'), job('src/b.ts', 'failed')];
  const events = [
    event('request_started', 'src/a.ts', 100),
    event('request_started', 'src/b.ts', 105),
    event('stream_chunk', 'src/a.ts', 120),
    event('job_failed', 'src/b.ts', 130, 'boom')
  ];
  const result = buildFileLogEntries(jobs, events);
  assert.equal(result.find((e) => e.relative_path === 'src/b.ts')?.error_message, 'boom');
  assert.equal(result.find((e) => e.relative_path === 'src/a.ts')?.phases.length, 2);
}

// 4. job_failed sets error_message to most recent failure
{
  const jobs = [job('src/x.ts', 'failed')];
  const events = [
    event('job_failed', 'src/x.ts', 100, 'first'),
    event('job_failed', 'src/x.ts', 200, 'second')
  ];
  const result = buildFileLogEntries(jobs, events);
  assert.equal(result[0].error_message, 'second');
}

// 5. Orphan event (path not in jobs) is silently dropped
{
  const result = buildFileLogEntries(
    [job('src/a.ts', 'done')],
    [
      event('request_started', 'src/a.ts', 100),
      event('request_started', 'src/ghost.ts', 110)
    ]
  );
  assert.equal(result.length, 1);
  assert.equal(result[0].relative_path, 'src/a.ts');
}

console.log('commenter file log PASSED');
```

- [ ] **Step 2: Run test to verify it fails**

```bash
pnpm exec tsx src/lib/commenterFileLog.test.ts
```

Expected: FAIL with module-not-found for `./commenterFileLog`.

- [ ] **Step 3: Write the minimal implementation**

Create `src/lib/commenterFileLog.ts`:

```ts
import type {
  CommenterEventKind,
  CommenterEventLevel,
  CommenterEventPayload,
  CommenterJobRecord,
  CommenterJobStatus
} from './commenterTypes';

export type PhaseTag =
  | 'queued'
  | 'requested'
  | 'first_chunk'
  | 'response_done'
  | 'validated'
  | 'written'
  | 'review_requested'
  | 'failed'
  | 'rolled_back';

export interface FileLogEntry {
  relative_path: string;
  status: CommenterJobStatus;
  phases: { phase: PhaseTag; at: number; level: CommenterEventLevel }[];
  started_at: number | null;
  ended_at: number | null;
  error_message: string | null;
}

const KIND_TO_PHASE: Partial<Record<CommenterEventKind, PhaseTag>> = {
  run_queued: 'queued',
  request_started: 'requested',
  stream_chunk: 'first_chunk',
  model_response_completed: 'response_done',
  job_updated: 'written',
  review_requested: 'review_requested',
  job_failed: 'failed',
  run_rolled_back: 'rolled_back'
};

export function buildFileLogEntries(
  jobs: CommenterJobRecord[],
  events: CommenterEventPayload[]
): FileLogEntry[] {
  const known = new Map<string, FileLogEntry>();
  for (const job of jobs) {
    known.set(job.relative_path, {
      relative_path: job.relative_path,
      status: job.status,
      phases: [],
      started_at: null,
      ended_at: null,
      error_message: null
    });
  }

  const sorted = [...events].sort((a, b) => a.created_at - b.created_at);
  const seen_first_chunk = new Set<string>();

  for (const event of sorted) {
    if (!event.relative_path) continue;
    const entry = known.get(event.relative_path);
    if (!entry) continue;

    const phase = KIND_TO_PHASE[event.kind];
    if (!phase) continue;

    if (phase === 'first_chunk') {
      if (seen_first_chunk.has(event.relative_path)) continue;
      seen_first_chunk.add(event.relative_path);
    }

    entry.phases.push({ phase, at: event.created_at, level: event.level });
    if (entry.started_at === null) entry.started_at = event.created_at;
    entry.ended_at = event.created_at;

    if (event.kind === 'job_failed') {
      entry.error_message = event.message || entry.error_message;
    }
  }

  return jobs.map((job) => known.get(job.relative_path)!).filter(Boolean);
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
pnpm exec tsx src/lib/commenterFileLog.test.ts
```

Expected: `commenter file log PASSED`.

- [ ] **Step 5: Commit**

```bash
git add src/lib/commenterFileLog.ts src/lib/commenterFileLog.test.ts
git commit -m "feat(commenter): add buildFileLogEntries pure aggregation"
```

---

## Task 2: Stream-slice reducer with LRU + cap

**Files:**
- Create: `src/lib/commenterStreamSlice.ts`
- Test: `src/lib/commenterStreamSlice.test.ts`

- [ ] **Step 1: Write the failing test**

Create `src/lib/commenterStreamSlice.test.ts`:

```ts
import assert from 'node:assert/strict';

import {
  applyEventToStreamSlices,
  STREAM_SLICE_TEXT_CAP_BYTES,
  STREAM_SLICE_TEXT_KEEP_COUNT,
  type LiveStreamSlice
} from './commenterStreamSlice';
import type { CommenterEventPayload } from './commenterTypes';

function event(
  kind: CommenterEventPayload['kind'],
  relative_path: string,
  at: number,
  message = ''
): CommenterEventPayload {
  return {
    kind,
    run_key: 'r1',
    relative_path,
    level: kind === 'job_failed' ? 'error' : 'info',
    message,
    created_at: at
  };
}

function key(path: string): string {
  return `r1|${path}`;
}

// 1. request_started creates an empty slice in 'streaming'
{
  const next = applyEventToStreamSlices(new Map(), event('request_started', 'a.ts', 10));
  const slice = next.get(key('a.ts'))!;
  assert.equal(slice.text, '');
  assert.equal(slice.status, 'streaming');
  assert.equal(slice.started_at, 10);
}

// 2. stream_chunk events concatenate in order
{
  let map = new Map<string, LiveStreamSlice>();
  map = applyEventToStreamSlices(map, event('request_started', 'a.ts', 10));
  map = applyEventToStreamSlices(map, event('stream_chunk', 'a.ts', 20, 'hello '));
  map = applyEventToStreamSlices(map, event('stream_chunk', 'a.ts', 30, 'world'));
  assert.equal(map.get(key('a.ts'))!.text, 'hello world');
  assert.equal(map.get(key('a.ts'))!.last_chunk_at, 30);
}

// 3. model_response_completed flips status without touching text
{
  let map = new Map<string, LiveStreamSlice>();
  map = applyEventToStreamSlices(map, event('request_started', 'a.ts', 10));
  map = applyEventToStreamSlices(map, event('stream_chunk', 'a.ts', 20, 'hi'));
  map = applyEventToStreamSlices(map, event('model_response_completed', 'a.ts', 30));
  const slice = map.get(key('a.ts'))!;
  assert.equal(slice.text, 'hi');
  assert.equal(slice.status, 'completed');
}

// 4. job_failed sets status='failed' and error
{
  let map = new Map<string, LiveStreamSlice>();
  map = applyEventToStreamSlices(map, event('request_started', 'a.ts', 10));
  map = applyEventToStreamSlices(map, event('job_failed', 'a.ts', 20, 'boom'));
  const slice = map.get(key('a.ts'))!;
  assert.equal(slice.status, 'failed');
  assert.equal(slice.error, 'boom');
}

// 5. LRU: 31st streaming file evicts text from the oldest by last_chunk_at
{
  let map = new Map<string, LiveStreamSlice>();
  for (let i = 0; i < STREAM_SLICE_TEXT_KEEP_COUNT + 1; i += 1) {
    const path = `f${i}.ts`;
    map = applyEventToStreamSlices(map, event('request_started', path, 100 + i));
    map = applyEventToStreamSlices(map, event('stream_chunk', path, 200 + i, 'data'));
  }
  const oldest = map.get(key('f0.ts'))!;
  assert.equal(oldest.text, '', 'oldest slice should have its text cleared');
  assert.equal(oldest.status, 'streaming', 'metadata is retained');
  const newest = map.get(key(`f${STREAM_SLICE_TEXT_KEEP_COUNT}.ts`))!;
  assert.equal(newest.text, 'data');
}

// 6. 5MB cap stops appending and inserts truncation marker
{
  let map = new Map<string, LiveStreamSlice>();
  map = applyEventToStreamSlices(map, event('request_started', 'big.ts', 10));
  const big = 'x'.repeat(STREAM_SLICE_TEXT_CAP_BYTES);
  map = applyEventToStreamSlices(map, event('stream_chunk', 'big.ts', 20, big));
  map = applyEventToStreamSlices(map, event('stream_chunk', 'big.ts', 30, 'extra'));
  const slice = map.get(key('big.ts'))!;
  assert.ok(slice.text.includes('truncated'), 'should contain truncation marker');
  assert.ok(!slice.text.includes('extra'), 'further chunks must be dropped');
}

console.log('commenter stream slice PASSED');
```

- [ ] **Step 2: Run test to verify it fails**

```bash
pnpm exec tsx src/lib/commenterStreamSlice.test.ts
```

Expected: FAIL with module-not-found.

- [ ] **Step 3: Write the minimal implementation**

Create `src/lib/commenterStreamSlice.ts`:

```ts
import type { CommenterEventPayload } from './commenterTypes';

export const STREAM_SLICE_TEXT_KEEP_COUNT = 30;
export const STREAM_SLICE_TEXT_CAP_BYTES = 5 * 1024 * 1024;
const TRUNCATION_MARKER = '\n[... truncated, full text in candidate.txt]\n';

export interface LiveStreamSlice {
  text: string;
  started_at: number;
  last_chunk_at: number;
  status: 'streaming' | 'completed' | 'failed';
  error: string | null;
  truncated: boolean;
}

export function streamSliceKey(run_key: string, relative_path: string): string {
  return `${run_key}|${relative_path}`;
}

function emptySlice(at: number): LiveStreamSlice {
  return {
    text: '',
    started_at: at,
    last_chunk_at: at,
    status: 'streaming',
    error: null,
    truncated: false
  };
}

function applyChunk(slice: LiveStreamSlice, message: string, at: number): LiveStreamSlice {
  if (slice.truncated) {
    return { ...slice, last_chunk_at: at };
  }
  const remaining = STREAM_SLICE_TEXT_CAP_BYTES - slice.text.length;
  if (remaining <= 0) {
    return {
      ...slice,
      text: slice.text + TRUNCATION_MARKER,
      truncated: true,
      last_chunk_at: at
    };
  }
  if (message.length <= remaining) {
    return { ...slice, text: slice.text + message, last_chunk_at: at };
  }
  return {
    ...slice,
    text: slice.text + message.slice(0, remaining) + TRUNCATION_MARKER,
    truncated: true,
    last_chunk_at: at
  };
}

function evictOldestText(slices: Map<string, LiveStreamSlice>): Map<string, LiveStreamSlice> {
  const withText = [...slices.entries()].filter(([, slice]) => slice.text.length > 0);
  if (withText.length <= STREAM_SLICE_TEXT_KEEP_COUNT) return slices;

  withText.sort((a, b) => a[1].last_chunk_at - b[1].last_chunk_at);
  const drop_count = withText.length - STREAM_SLICE_TEXT_KEEP_COUNT;
  const next = new Map(slices);
  for (let i = 0; i < drop_count; i += 1) {
    const [k, slice] = withText[i];
    next.set(k, { ...slice, text: '', truncated: false });
  }
  return next;
}

export function applyEventToStreamSlices(
  current: Map<string, LiveStreamSlice>,
  event: CommenterEventPayload
): Map<string, LiveStreamSlice> {
  if (!event.relative_path) return current;
  const k = streamSliceKey(event.run_key, event.relative_path);
  const next = new Map(current);

  switch (event.kind) {
    case 'request_started':
      next.set(k, emptySlice(event.created_at));
      return evictOldestText(next);
    case 'stream_chunk': {
      const existing = next.get(k) ?? emptySlice(event.created_at);
      next.set(k, applyChunk(existing, event.message, event.created_at));
      return evictOldestText(next);
    }
    case 'model_response_completed': {
      const existing = next.get(k);
      if (!existing) return current;
      next.set(k, { ...existing, status: 'completed', last_chunk_at: event.created_at });
      return next;
    }
    case 'job_failed': {
      const existing = next.get(k) ?? emptySlice(event.created_at);
      next.set(k, {
        ...existing,
        status: 'failed',
        error: event.message || existing.error,
        last_chunk_at: event.created_at
      });
      return next;
    }
    default:
      return current;
  }
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
pnpm exec tsx src/lib/commenterStreamSlice.test.ts
```

Expected: `commenter stream slice PASSED`.

- [ ] **Step 5: Commit**

```bash
git add src/lib/commenterStreamSlice.ts src/lib/commenterStreamSlice.test.ts
git commit -m "feat(commenter): add live stream slice reducer with LRU and 5MB cap"
```

---

## Task 3: Backend `commenter_list_dir` command

**Files:**
- Modify: `src-tauri/src/commenter/commands.rs`

- [ ] **Step 1: Write the failing tests**

Append the following two test functions inside the existing `mod tests { ... }` block at the bottom of `src-tauri/src/commenter/commands.rs` (find the existing `mod tests` and add to it). Replace `<EXISTING_TESTS_MOD>` mentally with the existing module:

```rust
#[test]
fn list_dir_returns_entries_under_profile_root_and_filters_excludes() {
    let temp = tempfile::tempdir().expect("tempdir");
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
            settings: CommenterProjectSettings::default(),
        })
        .expect("upsert profile");

    let entries = service
        .list_dir(&profile.profile_key, "")
        .expect("list root");
    let names: std::collections::HashSet<String> = entries.iter().map(|e| e.name.clone()).collect();
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
    let temp = tempfile::tempdir().expect("tempdir");
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
            settings: CommenterProjectSettings::default(),
        })
        .expect("upsert profile");

    let outcome = service.list_dir(&profile.profile_key, "../../../etc");
    assert!(
        outcome.is_err(),
        "path traversal must be rejected, got {:?}",
        outcome
    );
}
```

If `tempfile` and `CommenterProjectSettings::default()` are not already in scope or available, examine the surrounding tests to mirror their setup patterns. The existing `command_surface_initializes_sqlite_app_database` test (around line 2066) is a good template.

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --manifest-path src-tauri/Cargo.toml commenter::commands::tests::list_dir
```

Expected: FAIL with `no method named list_dir`.

- [ ] **Step 3: Write the minimal implementation**

Add to `src-tauri/src/commenter/commands.rs`. First, near the other public surface types (around the top of the file, where serializable response types live), add:

```rust
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
```

Then add the method on `impl CommenterCommandSurface` (next to other `pub fn list_*` methods, around line 245):

```rust
pub fn list_dir(
    &self,
    profile_key: &str,
    relative_path: &str,
) -> Result<Vec<CommenterDirEntry>, String> {
    let profiles = self.list_project_profiles()?;
    let profile = profiles
        .iter()
        .find(|p| p.profile_key == profile_key)
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
        (CommenterDirEntryKind::File, CommenterDirEntryKind::Dir) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(entries)
}
```

Add the `#[tauri::command]` wrapper at the bottom of the file (next to other wrappers, around line 1900):

```rust
#[tauri::command]
pub fn commenter_list_dir(
    surface: tauri::State<'_, CommenterCommandSurface>,
    profile_key: String,
    relative_path: String,
) -> Result<Vec<CommenterDirEntry>, String> {
    surface.list_dir(&profile_key, &relative_path)
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test --manifest-path src-tauri/Cargo.toml commenter::commands::tests::list_dir
```

Expected: both `list_dir_*` tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commenter/commands.rs
git commit -m "feat(commenter): add list_dir command for workspace tree lazy load"
```

---

## Task 4: Backend `commenter_get_candidate_text` command

**Files:**
- Modify: `src-tauri/src/commenter/commands.rs`

- [ ] **Step 1: Write the failing tests**

Append inside the existing `mod tests`:

```rust
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
fn get_candidate_text_returns_empty_when_artifact_missing() {
    let temp = tempfile::tempdir().expect("tempdir");
    let data_root = temp.path().join(".commenter-data");
    let service = CommenterCommandSurface::new(&data_root);

    let text = service
        .get_candidate_text("nonexistent-run", "missing/file.ts")
        .expect("missing artifact returns Ok");
    assert_eq!(text, "");
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --manifest-path src-tauri/Cargo.toml commenter::commands::tests::get_candidate_text
```

Expected: FAIL with `no method named get_candidate_text`.

- [ ] **Step 3: Write the minimal implementation**

Add to `impl CommenterCommandSurface`:

```rust
pub fn get_candidate_text(&self, run_key: &str, relative_path: &str) -> Result<String, String> {
    let run_paths = crate::commenter::artifacts::CommenterRunPaths::new(self.data_root(), run_key)
        .map_err(|e| e.to_string())?;
    let candidate_path = run_paths
        .candidate_root
        .join(relative_path)
        .with_file_name(format!(
            "{}.candidate",
            std::path::Path::new(relative_path)
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("artifact")
        ));

    let canonical_root = run_paths
        .candidate_root
        .canonicalize()
        .unwrap_or_else(|_| run_paths.candidate_root.clone());
    let canonical_target = match candidate_path.canonicalize() {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(error) => return Err(error.to_string()),
    };
    if !canonical_target.starts_with(&canonical_root) {
        return Err("candidate path escapes run root".to_string());
    }

    match std::fs::read_to_string(&canonical_target) {
        Ok(content) => Ok(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error.to_string()),
    }
}
```

If `CommenterCommandSurface` does not already expose its `data_root` via a getter, add:

```rust
pub fn data_root(&self) -> &std::path::Path {
    &self.data_root
}
```

(replace `&self.data_root` with the actual private field name; check the struct definition near line 174.)

Add the wrapper:

```rust
#[tauri::command]
pub fn commenter_get_candidate_text(
    surface: tauri::State<'_, CommenterCommandSurface>,
    run_key: String,
    relative_path: String,
) -> Result<String, String> {
    surface.get_candidate_text(&run_key, &relative_path)
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test --manifest-path src-tauri/Cargo.toml commenter::commands::tests::get_candidate_text
```

Expected: both `get_candidate_text_*` tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commenter/commands.rs
git commit -m "feat(commenter): add get_candidate_text command for locked stream view"
```

---

## Task 5: Register commands and expose them via the frontend API

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/tauri.ts`
- Modify: `src/lib/mockCommenterBackend.ts`

- [ ] **Step 1: Register the new commands in `lib.rs`**

In `src-tauri/src/lib.rs`, append two entries to the `tauri::generate_handler![]` list:

```rust
.invoke_handler(tauri::generate_handler![
    commenter::commands::commenter_upsert_project_profile,
    commenter::commands::commenter_list_project_profiles,
    commenter::commands::commenter_enqueue_run,
    commenter::commands::commenter_list_runs,
    commenter::commands::commenter_get_run_detail,
    commenter::commands::commenter_delete_run,
    commenter::commands::commenter_start_run,
    commenter::commands::commenter_pause_run,
    commenter::commands::commenter_resume_run,
    commenter::commands::commenter_cancel_run,
    commenter::commands::commenter_list_review_jobs,
    commenter::commands::commenter_accept_review_job,
    commenter::commands::commenter_reject_review_job,
    commenter::commands::commenter_retry_job,
    commenter::commands::commenter_open_external_diff,
    commenter::commands::commenter_rollback_run,
    commenter::commands::commenter_get_app_settings,
    commenter::commands::commenter_update_app_settings,
    commenter::commands::commenter_get_diff_tool_settings,
    commenter::commands::commenter_update_diff_tool_settings,
    commenter::commands::commenter_list_dir,
    commenter::commands::commenter_get_candidate_text
])
```

- [ ] **Step 2: Add types and API methods to the frontend bridge**

In `src/lib/commenterTypes.ts`, append:

```ts
export type CommenterDirEntryKind = 'dir' | 'file';

export interface CommenterDirEntry {
  name: string;
  kind: CommenterDirEntryKind;
  relative_path: string;
}
```

In `src/lib/tauri.ts`, add to the `commenterApi` object (just before the closing `};`):

```ts
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
},
```

Also import `CommenterDirEntry` at the top of `tauri.ts`.

- [ ] **Step 3: Add stub responses to `mockCommenterBackend.ts`**

In `src/lib/mockCommenterBackend.ts`, add these methods to the exported object:

```ts
listDir: async (_profile_key: string, _relative_path: string): Promise<CommenterDirEntry[]> => [],
getCandidateText: async (_run_key: string, _relative_path: string): Promise<string> => ''
```

(Import `CommenterDirEntry` at the top.)

- [ ] **Step 4: Verify the wiring builds**

```bash
pnpm check
```

Expected: vue-tsc passes, all existing smoke tests pass.

```bash
cargo build --manifest-path src-tauri/Cargo.toml
```

Expected: clean build with no warnings about unused commands.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs src/lib/tauri.ts src/lib/commenterTypes.ts src/lib/mockCommenterBackend.ts
git commit -m "feat(commenter): wire list_dir and get_candidate_text into frontend bridge"
```

---

## Task 6: Integrate stream-slice reducer into `commenterStore`

**Files:**
- Modify: `src/lib/commenterStore.ts`

- [ ] **Step 1: Extend the store state and event handler**

At the top of `src/lib/commenterStore.ts`, add the import:

```ts
import {
  applyEventToStreamSlices,
  streamSliceKey,
  type LiveStreamSlice
} from './commenterStreamSlice';
```

Inside `interface CommenterStoreState`, add the new field:

```ts
live_streams: Map<string, LiveStreamSlice>;
```

In the `reactive<CommenterStoreState>({...})` initializer, add:

```ts
live_streams: new Map(),
```

Replace `appendExecutionEvent` with:

```ts
function appendExecutionEvent(event: CommenterEventPayload) {
  mergeExecutionLogs([event]);
  mergeDetailEvent(event);
  state.live_streams = applyEventToStreamSlices(state.live_streams, event);
}
```

Inside `selectRun`, clear the live streams map before the refresh call:

```ts
selectRun(run_key: string) {
  state.selected_run_key = run_key;
  state.live_streams = new Map();
  return runWithRefresh(async () => undefined);
},
```

Inside `deleteRun`, drop slices for the deleted run after the existing event filter:

```ts
async deleteRun(run_key: string) {
  return runWithRefresh(async () => {
    const deleted = await commenterApi.deleteRun(run_key);
    state.execution_logs = state.execution_logs.filter((event) => event.run_key !== run_key);
    const next_streams = new Map(state.live_streams);
    for (const key of [...next_streams.keys()]) {
      if (key.startsWith(`${run_key}|`)) next_streams.delete(key);
    }
    state.live_streams = next_streams;
    if (state.selected_run_key === run_key) {
      state.selected_run_key = null;
      state.selected_run_detail = null;
    }
    return deleted;
  });
},
```

- [ ] **Step 2: Add a smoke assertion that the store wires up the slice**

Add to the existing `src/lib/commenterApiShape.test.ts` (read it first to see the assertion style — it's `assert.match(source, /…/)` against the file). Append:

```ts
const store_source = fs.readFileSync(new URL('./commenterStore.ts', import.meta.url), 'utf8');
assert.match(store_source, /applyEventToStreamSlices/, 'store should pipe events through stream slice reducer');
assert.match(store_source, /live_streams: new Map\(\)/, 'store should initialize live_streams');
assert.match(store_source, /state\.live_streams = new Map\(\)/, 'selectRun should clear live_streams');
```

- [ ] **Step 3: Run `pnpm smoke` to verify**

```bash
pnpm smoke
```

Expected: all existing scripts pass plus the new assertions on `commenterApiShape.test.ts`.

- [ ] **Step 4: Run `pnpm check`**

```bash
pnpm check
```

Expected: vue-tsc clean.

- [ ] **Step 5: Commit**

```bash
git add src/lib/commenterStore.ts src/lib/commenterApiShape.test.ts
git commit -m "feat(commenter): wire live stream slice reducer into store"
```

---

## Task 7: `RunHeaderStrip.vue`

**Files:**
- Create: `src/components/commenter/RunHeaderStrip.vue`
- Create: `src/lib/commenterRunHeader.test.ts`
- Modify: `package.json` (append the new test to `smoke`)
- Modify: `src/locales/messages.ts`

- [ ] **Step 1: Add new translation keys**

In `src/locales/messages.ts`, add to both the `en` and `zh` (or whichever locales the file defines) maps:

```ts
'commenter.header.title': 'Run',
'commenter.header.pause': 'Pause',
'commenter.header.resume': 'Resume',
'commenter.header.cancel': 'Cancel',
'commenter.header.idle': 'No run selected',
'commenter.header.progress': 'Progress',
```

(Use Chinese equivalents in the zh map, e.g. `'commenter.header.pause': '暂停'`.)

- [ ] **Step 2: Write the failing contract test**

Create `src/lib/commenterRunHeader.test.ts`:

```ts
import assert from 'node:assert/strict';
import fs from 'node:fs';

const source = fs.readFileSync(
  new URL('../components/commenter/RunHeaderStrip.vue', import.meta.url),
  'utf8'
);

assert.match(source, /pauseRun/, 'header should call pauseRun on pause click');
assert.match(source, /resumeRun/, 'header should call resumeRun on resume click');
assert.match(source, /cancelRun/, 'header should call cancelRun on cancel click');
assert.match(source, /run_status_label/, 'header should render run status label');
assert.match(source, /selected_run_detail/, 'header should read selected_run_detail from store');
assert.match(source, /current_file/, 'header should render the current file');

console.log('commenter run header PASSED');
```

Add the file to `package.json` `smoke`:

```json
"smoke": "tsx src/lib/commenterApiShape.test.ts && tsx src/lib/commenterRoute.test.ts && tsx src/lib/commenterLocale.test.ts && tsx src/lib/commenterProfileDefaults.test.ts && tsx src/lib/settingsWorkspaceCleanup.test.ts && tsx src/lib/settingsWorkspaceEnhancements.test.ts && tsx src/lib/commenterExecutionLog.test.ts && tsx src/lib/commenterFileLog.test.ts && tsx src/lib/commenterStreamSlice.test.ts && tsx src/lib/commenterRunHeader.test.ts"
```

- [ ] **Step 3: Run smoke to verify it fails**

```bash
pnpm smoke
```

Expected: FAIL with file-not-found for `RunHeaderStrip.vue`.

- [ ] **Step 4: Implement the component**

Create `src/components/commenter/RunHeaderStrip.vue`:

```vue
<script setup lang="ts">
import { computed } from 'vue';
import { Pause, Play, X } from 'lucide-vue-next';

import { commenterStore } from '../../lib/commenterStore';
import { run_progress_percent, run_status_label } from '../../lib/commenterView';
import { use_messages } from '../../locales/messages';

const { t } = use_messages();

const detail = computed(() => commenterStore.state.selected_run_detail);
const run = computed(() => detail.value?.run ?? null);
const status = computed(() => run.value?.status ?? null);
const progress = computed(() => (run.value ? run_progress_percent(run.value) : 0));

const can_pause = computed(() => status.value === 'running');
const can_resume = computed(() => status.value === 'paused');
const can_cancel = computed(
  () => status.value === 'running' || status.value === 'paused' || status.value === 'pausing'
);

async function onPause() {
  if (run.value) await commenterStore.pauseRun(run.value.run_key);
}

async function onResume() {
  if (run.value) await commenterStore.resumeRun(run.value.run_key);
}

async function onCancel() {
  if (run.value) await commenterStore.cancelRun(run.value.run_key);
}
</script>

<template>
  <header
    v-if="run"
    class="run-header-strip"
    :data-status="status"
  >
    <div class="run-header-meta">
      <span class="run-header-label">{{ t('commenter.header.title') }}</span>
      <strong>{{ run.run_key }}</strong>
      <span class="run-header-status">{{ run_status_label(run.status) }}</span>
    </div>

    <div class="run-header-progress">
      <span class="run-header-progress-label">{{ t('commenter.header.progress') }}</span>
      <div class="run-header-progress-track">
        <span :style="{ width: `${progress}%` }" />
      </div>
      <span class="run-header-progress-value">
        {{ run.completed_jobs }} / {{ run.total_jobs }}
        <small v-if="run.review_needed_jobs > 0">⚠ {{ run.review_needed_jobs }}</small>
        <small v-if="run.failed_jobs > 0">✗ {{ run.failed_jobs }}</small>
      </span>
    </div>

    <div class="run-header-current">
      <span>{{ run.current_file ?? '—' }}</span>
    </div>

    <div class="run-header-actions">
      <button
        v-if="can_pause"
        type="button"
        @click="onPause"
      >
        <Pause :size="14" /> {{ t('commenter.header.pause') }}
      </button>
      <button
        v-if="can_resume"
        type="button"
        @click="onResume"
      >
        <Play :size="14" /> {{ t('commenter.header.resume') }}
      </button>
      <button
        v-if="can_cancel"
        type="button"
        class="run-header-cancel"
        @click="onCancel"
      >
        <X :size="14" /> {{ t('commenter.header.cancel') }}
      </button>
    </div>
  </header>

  <header
    v-else
    class="run-header-strip run-header-strip--idle"
  >
    <span>{{ t('commenter.header.idle') }}</span>
  </header>
</template>

<style scoped>
.run-header-strip {
  display: grid;
  grid-template-columns: auto 1fr auto auto;
  gap: 16px;
  align-items: center;
  padding: 10px 14px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  background: rgba(99, 102, 241, 0.08);
}

.run-header-strip--idle {
  grid-template-columns: 1fr;
  color: #88a5a1;
  background: rgba(255, 255, 255, 0.02);
}

.run-header-meta {
  display: flex;
  gap: 8px;
  align-items: baseline;
}

.run-header-label {
  color: #88a5a1;
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.run-header-status {
  background: rgba(34, 197, 94, 0.18);
  border-radius: 999px;
  padding: 2px 10px;
  font-size: 12px;
}

.run-header-progress {
  display: grid;
  gap: 4px;
}

.run-header-progress-track {
  height: 6px;
  border-radius: 3px;
  background: rgba(255, 255, 255, 0.08);
  overflow: hidden;
}

.run-header-progress-track span {
  display: block;
  height: 100%;
  background: #22c55e;
}

.run-header-current {
  font-family: ui-monospace, Menlo, Consolas, monospace;
  font-size: 12px;
  color: #d9ece9;
  max-width: 260px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.run-header-actions {
  display: flex;
  gap: 8px;
}

.run-header-actions button {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 6px 12px;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.1);
  color: inherit;
  cursor: pointer;
}

.run-header-actions .run-header-cancel {
  background: rgba(231, 111, 111, 0.16);
  border-color: rgba(231, 111, 111, 0.4);
}
</style>
```

- [ ] **Step 5: Run smoke to verify it passes**

```bash
pnpm smoke
```

Expected: `commenter run header PASSED` and all other suites green.

- [ ] **Step 6: Commit**

```bash
git add src/components/commenter/RunHeaderStrip.vue src/lib/commenterRunHeader.test.ts package.json src/locales/messages.ts
git commit -m "feat(commenter): add RunHeaderStrip component with pause/resume/cancel"
```

---

## Task 8: Rewrite `ExecutionLogPanel.vue` (compact per-file rows)

**Files:**
- Modify (rewrite): `src/components/commenter/ExecutionLogPanel.vue`
- Modify: `src/lib/commenterExecutionLog.test.ts`
- Modify: `src/locales/messages.ts`

- [ ] **Step 1: Update translations**

Append to `src/locales/messages.ts`:

```ts
'commenter.log.empty': 'No activity yet',
'commenter.log.expand': 'Show phases',
'commenter.log.collapse': 'Hide phases',
'commenter.log.phase.queued': 'Queued',
'commenter.log.phase.requested': 'Request sent',
'commenter.log.phase.first_chunk': 'First chunk',
'commenter.log.phase.response_done': 'Response complete',
'commenter.log.phase.written': 'Written',
'commenter.log.phase.review_requested': 'Review requested',
'commenter.log.phase.failed': 'Failed',
'commenter.log.phase.rolled_back': 'Rolled back',
```

(Mirror in zh.)

- [ ] **Step 2: Update the existing contract test**

Replace the body of `src/lib/commenterExecutionLog.test.ts` with:

```ts
import assert from 'node:assert/strict';
import fs from 'node:fs';

const queue_panel = new URL('../components/commenter/QueueRunsTable.vue', import.meta.url);
const log_panel = new URL('../components/commenter/ExecutionLogPanel.vue', import.meta.url);
const store_file = new URL('./commenterStore.ts', import.meta.url);
const tauri_file = new URL('./tauri.ts', import.meta.url);
const types_file = new URL('./commenterTypes.ts', import.meta.url);

const queue_source = fs.readFileSync(queue_panel, 'utf8');
assert.match(queue_source, /Trash2/, 'queue rows should expose a delete icon action');
assert.match(queue_source, /deleteRun/, 'queue rows should call the store delete action');

const log_source = fs.readFileSync(log_panel, 'utf8');
assert.match(log_source, /buildFileLogEntries/, 'execution log should derive entries via buildFileLogEntries');
assert.match(log_source, /select-file/, 'execution log should emit select-file when a row is clicked');
assert.match(log_source, /toggle/, 'execution log should toggle phase expansion');

const store_source = fs.readFileSync(store_file, 'utf8');
assert.match(store_source, /execution_logs/, 'store should retain live execution log entries');
assert.match(store_source, /subscribeCommenterEvents/, 'store should subscribe to backend execution events');

const tauri_source = fs.readFileSync(tauri_file, 'utf8');
assert.match(tauri_source, /commenter_delete_run/, 'Tauri API should expose run deletion');
assert.match(tauri_source, /commenter:\/\/state/, 'Tauri API should listen to commenter state events');

const types_source = fs.readFileSync(types_file, 'utf8');
for (const kind of ['request_started', 'stream_chunk', 'model_response_completed']) {
  assert.equal(types_source.includes(`'${kind}'`), true, `${kind} should be typed as an event kind`);
}

console.log('commenter execution log PASSED');
```

- [ ] **Step 3: Run smoke to verify the new assertions fail**

```bash
pnpm smoke
```

Expected: FAIL on `buildFileLogEntries` assertion.

- [ ] **Step 4: Rewrite the component**

Replace `src/components/commenter/ExecutionLogPanel.vue` entirely with:

```vue
<script setup lang="ts">
import { computed, ref } from 'vue';
import {
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  CircleDashed,
  Loader2,
  TriangleAlert,
  XCircle,
  Zap
} from 'lucide-vue-next';

import { commenterStore } from '../../lib/commenterStore';
import { buildFileLogEntries, type FileLogEntry, type PhaseTag } from '../../lib/commenterFileLog';
import { run_status_label } from '../../lib/commenterView';
import { use_messages } from '../../locales/messages';

const emit = defineEmits<{ (event: 'select-file', relative_path: string): void }>();

const { t } = use_messages();

const detail = computed(() => commenterStore.state.selected_run_detail);

const entries = computed<FileLogEntry[]>(() => {
  if (!detail.value) return [];
  return buildFileLogEntries(detail.value.jobs, detail.value.events);
});

const expanded = ref<Set<string>>(new Set());

function toggle(path: string) {
  const next = new Set(expanded.value);
  if (next.has(path)) next.delete(path);
  else next.add(path);
  expanded.value = next;
}

function statusIcon(entry: FileLogEntry) {
  switch (entry.status) {
    case 'done':
      return CheckCircle2;
    case 'failed':
      return XCircle;
    case 'review_needed':
      return TriangleAlert;
    case 'requesting':
    case 'validating':
    case 'writing':
      return Zap;
    case 'pending':
    case 'leased':
    case 'retry_waiting':
      return CircleDashed;
    default:
      return Loader2;
  }
}

function elapsed(entry: FileLogEntry): string {
  if (entry.started_at === null || entry.ended_at === null) return '';
  const ms = entry.ended_at - entry.started_at;
  if (ms <= 0) return '';
  return ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} s`;
}

function phaseLabel(phase: PhaseTag): string {
  return t(`commenter.log.phase.${phase}`);
}

function onSelect(entry: FileLogEntry) {
  emit('select-file', entry.relative_path);
}
</script>

<template>
  <section class="execution-log-panel">
    <header class="execution-log-header">
      <h3>{{ t('commenter.logs') }}</h3>
      <span v-if="detail">{{ run_status_label(detail.run.status) }}</span>
    </header>

    <ul
      v-if="entries.length > 0"
      class="execution-log-list"
    >
      <li
        v-for="entry in entries"
        :key="entry.relative_path"
        class="execution-log-row"
        :class="`execution-log-row--${entry.status}`"
      >
        <button
          type="button"
          class="execution-log-toggle"
          :aria-expanded="expanded.has(entry.relative_path)"
          @click="toggle(entry.relative_path)"
        >
          <component :is="expanded.has(entry.relative_path) ? ChevronDown : ChevronRight" :size="12" />
        </button>
        <button
          type="button"
          class="execution-log-row-main"
          @click="onSelect(entry)"
        >
          <component :is="statusIcon(entry)" :size="14" />
          <span class="execution-log-path">{{ entry.relative_path }}</span>
          <span class="execution-log-elapsed">{{ elapsed(entry) }}</span>
        </button>
        <ul
          v-if="expanded.has(entry.relative_path)"
          class="execution-log-phases"
        >
          <li
            v-for="phase in entry.phases"
            :key="`${phase.phase}-${phase.at}`"
            :class="`execution-log-phase execution-log-phase--${phase.level}`"
          >
            <span>{{ phaseLabel(phase.phase) }}</span>
            <small>{{ new Date(phase.at).toLocaleTimeString() }}</small>
          </li>
          <li
            v-if="entry.error_message"
            class="execution-log-phase execution-log-phase--error"
          >
            <span>{{ entry.error_message }}</span>
          </li>
        </ul>
      </li>
    </ul>

    <div
      v-else
      class="empty-state"
    >
      {{ t('commenter.log.empty') }}
    </div>
  </section>
</template>

<style scoped>
.execution-log-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.18);
}

.execution-log-header {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  padding: 10px 12px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}

.execution-log-header h3 {
  margin: 0;
  font-size: 13px;
}

.execution-log-list {
  list-style: none;
  margin: 0;
  padding: 6px;
  overflow: auto;
  flex: 1;
  min-height: 0;
}

.execution-log-row {
  display: grid;
  grid-template-columns: 18px 1fr;
  align-items: center;
  gap: 4px;
  padding: 4px 6px;
  border-radius: 6px;
  font-size: 12px;
}

.execution-log-row--done {
  background: rgba(34, 197, 94, 0.06);
}
.execution-log-row--failed {
  background: rgba(231, 111, 111, 0.1);
}
.execution-log-row--review_needed {
  background: rgba(245, 158, 11, 0.1);
}
.execution-log-row--requesting,
.execution-log-row--validating,
.execution-log-row--writing {
  background: rgba(99, 102, 241, 0.12);
}

.execution-log-toggle,
.execution-log-row-main {
  background: none;
  border: 0;
  color: inherit;
  cursor: pointer;
  text-align: left;
  padding: 0;
}

.execution-log-row-main {
  display: grid;
  grid-template-columns: 16px 1fr auto;
  align-items: center;
  gap: 6px;
  font-family: ui-monospace, Menlo, Consolas, monospace;
}

.execution-log-path {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.execution-log-elapsed {
  color: #88a5a1;
  font-size: 10px;
}

.execution-log-phases {
  grid-column: 2 / span 1;
  list-style: none;
  margin: 4px 0 0;
  padding: 0;
  display: grid;
  gap: 2px;
  font-size: 11px;
}

.execution-log-phase {
  display: flex;
  justify-content: space-between;
  color: #b8c9c6;
}

.execution-log-phase--error {
  color: #f87171;
}
</style>
```

- [ ] **Step 5: Run smoke and check**

```bash
pnpm smoke
pnpm check
```

Expected: all suites pass.

- [ ] **Step 6: Commit**

```bash
git add src/components/commenter/ExecutionLogPanel.vue src/lib/commenterExecutionLog.test.ts src/locales/messages.ts
git commit -m "refactor(commenter): rewrite ExecutionLogPanel as compact per-file rows"
```

---

## Task 9: `WorkspaceTreePanel.vue`

**Files:**
- Create: `src/components/commenter/WorkspaceTreePanel.vue`
- Create: `src/lib/commenterWorkspaceTree.test.ts`
- Modify: `package.json`
- Modify: `src/locales/messages.ts`

- [ ] **Step 1: Add translations**

```ts
'commenter.tree.empty': 'No queued files for this run',
'commenter.tree.error.root': 'Project root unreachable — check the profile root path.',
'commenter.tree.error.dir': 'Cannot read directory',
'commenter.tree.retry': 'Retry',
```

- [ ] **Step 2: Add the contract test**

Create `src/lib/commenterWorkspaceTree.test.ts`:

```ts
import assert from 'node:assert/strict';
import fs from 'node:fs';

const source = fs.readFileSync(
  new URL('../components/commenter/WorkspaceTreePanel.vue', import.meta.url),
  'utf8'
);

assert.match(source, /commenterApi\.listDir/, 'tree should call commenterApi.listDir');
assert.match(source, /current_file/, 'tree should reference current_file for auto-expand');
assert.match(source, /queued_paths/, 'tree should highlight queued paths');
assert.match(source, /select-file/, 'tree should emit select-file');

console.log('commenter workspace tree PASSED');
```

Append to the smoke script:

```
&& tsx src/lib/commenterWorkspaceTree.test.ts
```

- [ ] **Step 3: Run smoke to verify it fails**

```bash
pnpm smoke
```

Expected: FAIL with file-not-found.

- [ ] **Step 4: Implement the component**

Create `src/components/commenter/WorkspaceTreePanel.vue`:

```vue
<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import { AlertTriangle } from 'lucide-vue-next';

import { commenterApi } from '../../lib/tauri';
import type { CommenterDirEntry } from '../../lib/commenterTypes';
import { commenterStore } from '../../lib/commenterStore';
import { use_messages } from '../../locales/messages';

interface TreeNode {
  kind: 'dir' | 'file';
  name: string;
  relative_path: string;
  children: TreeNode[] | null;
  expanded: boolean;
  loading: boolean;
  error: string | null;
}

const emit = defineEmits<{ (event: 'select-file', relative_path: string): void }>();

const { t } = use_messages();

const detail = computed(() => commenterStore.state.selected_run_detail);
const profile_key = computed(() => detail.value?.run.profile_key ?? null);
const queued_paths = computed(
  () => new Set(detail.value?.jobs.map((j) => j.relative_path) ?? [])
);
const current_file = computed(() => detail.value?.run.current_file ?? null);

const root = reactive<TreeNode>({
  kind: 'dir',
  name: '',
  relative_path: '',
  children: null,
  expanded: true,
  loading: false,
  error: null
});

const root_error = ref<string | null>(null);

async function load(node: TreeNode) {
  if (!profile_key.value) return;
  if (node.children) return;
  node.loading = true;
  node.error = null;
  try {
    const entries: CommenterDirEntry[] = await commenterApi.listDir(
      profile_key.value,
      node.relative_path
    );
    node.children = entries.map((entry) => ({
      kind: entry.kind,
      name: entry.name,
      relative_path: entry.relative_path,
      children: null,
      expanded: false,
      loading: false,
      error: null
    }));
  } catch (error) {
    node.error = error instanceof Error ? error.message : String(error);
    if (node === root) root_error.value = node.error;
  } finally {
    node.loading = false;
  }
}

async function toggle(node: TreeNode) {
  if (node.kind === 'file') {
    if (queued_paths.value.has(node.relative_path)) emit('select-file', node.relative_path);
    return;
  }
  node.expanded = !node.expanded;
  if (node.expanded && !node.children) await load(node);
}

async function expandAncestors(path: string) {
  const segments = path.split('/').filter(Boolean);
  let cursor: TreeNode | undefined = root;
  let traversed = '';
  for (const segment of segments) {
    if (!cursor) return;
    if (!cursor.children) await load(cursor);
    if (!cursor.children) return;
    traversed = traversed ? `${traversed}/${segment}` : segment;
    const next: TreeNode | undefined = cursor.children.find((child) => child.relative_path === traversed);
    if (!next) return;
    if (next.kind === 'dir') {
      next.expanded = true;
      if (!next.children) await load(next);
    }
    cursor = next;
  }
}

watch(
  () => profile_key.value,
  async (key) => {
    root.children = null;
    root.expanded = true;
    root.error = null;
    root_error.value = null;
    if (key) await load(root);
  },
  { immediate: true }
);

watch(
  () => current_file.value,
  async (path) => {
    if (path) await expandAncestors(path);
  }
);
</script>

<template>
  <section class="workspace-tree-panel">
    <header>
      <h3>{{ t('commenter.detail') }}</h3>
    </header>
    <div
      v-if="root_error"
      class="tree-error"
    >
      <AlertTriangle :size="14" />
      <span>{{ t('commenter.tree.error.root') }}</span>
      <small>{{ root_error }}</small>
    </div>
    <div
      v-else-if="!profile_key"
      class="empty-state"
    >
      {{ t('commenter.tree.empty') }}
    </div>
    <ul
      v-else
      class="tree"
    >
      <TreeNodeRender
        v-for="child in root.children ?? []"
        :key="child.relative_path"
        :node="child"
        :queued-paths="queued_paths"
        :current-file="current_file"
        @toggle="toggle"
      />
    </ul>
  </section>
</template>

<script lang="ts">
import { defineComponent, h, type PropType } from 'vue';
import { ChevronDown as TNChevronDown, ChevronRight as TNChevronRight, FileCode as TNFileCode, Folder as TNFolder, FolderOpen as TNFolderOpen } from 'lucide-vue-next';

interface RenderNode {
  kind: 'dir' | 'file';
  name: string;
  relative_path: string;
  children: RenderNode[] | null;
  expanded: boolean;
  loading: boolean;
  error: string | null;
}

const TreeNodeRender = defineComponent({
  name: 'TreeNodeRender',
  props: {
    node: { type: Object as PropType<RenderNode>, required: true },
    queuedPaths: { type: Object as PropType<Set<string>>, required: true },
    currentFile: { type: String as PropType<string | null>, default: null }
  },
  emits: ['toggle'],
  setup(props, { emit }) {
    const queued = () => props.queuedPaths.has(props.node.relative_path);
    return () => {
      const node = props.node;
      const is_current = node.relative_path === props.currentFile;
      return h(
        'li',
        { class: ['tree-node', { 'tree-node--queued': queued(), 'tree-node--current': is_current }] },
        [
          h(
            'button',
            {
              type: 'button',
              class: 'tree-node-row',
              onClick: () => emit('toggle', node)
            },
            [
              node.kind === 'dir'
                ? h(node.expanded ? TNChevronDown : TNChevronRight, { size: 12 })
                : h('span', { style: { width: '12px', display: 'inline-block' } }),
              node.kind === 'dir'
                ? h(node.expanded ? TNFolderOpen : TNFolder, { size: 14 })
                : h(TNFileCode, { size: 14 }),
              h('span', { class: 'tree-node-name' }, node.name)
            ]
          ),
          node.expanded && node.children
            ? h(
                'ul',
                { class: 'tree' },
                node.children.map((child) =>
                  h(TreeNodeRender, {
                    key: child.relative_path,
                    node: child,
                    queuedPaths: props.queuedPaths,
                    currentFile: props.currentFile,
                    onToggle: (target: RenderNode) => emit('toggle', target)
                  })
                )
              )
            : null
        ]
      );
    };
  }
});

export { TreeNodeRender };
</script>

<style scoped>
.workspace-tree-panel {
  display: flex;
  flex-direction: column;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.18);
  height: 100%;
}

.workspace-tree-panel header {
  padding: 10px 12px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}

.tree {
  list-style: none;
  margin: 0;
  padding: 6px;
  overflow: auto;
  flex: 1;
}

.tree-node-row {
  display: flex;
  align-items: center;
  gap: 4px;
  width: 100%;
  background: none;
  border: 0;
  padding: 2px 4px;
  color: inherit;
  font-family: ui-monospace, Menlo, Consolas, monospace;
  font-size: 12px;
  text-align: left;
  cursor: pointer;
  border-radius: 4px;
}

.tree-node:not(.tree-node--queued) > .tree-node-row {
  opacity: 0.5;
  cursor: default;
}

.tree-node--current > .tree-node-row {
  background: rgba(99, 102, 241, 0.18);
  font-weight: 600;
}

.tree-error {
  display: grid;
  gap: 6px;
  padding: 12px;
  color: #f87171;
}

.tree-error small {
  color: #88a5a1;
  font-family: ui-monospace, Menlo, Consolas, monospace;
}
</style>
```

> **Note for the implementer:** Vue 3 SFCs typically have a single `<script setup>` block. Above we use a second `<script>` block to register a recursive helper component. If the project's lint config rejects this, factor `TreeNodeRender` into its own `WorkspaceTreeNode.vue` SFC and import it. Either approach satisfies the contract test.

- [ ] **Step 5: Run smoke**

```bash
pnpm smoke
pnpm check
```

Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add src/components/commenter/WorkspaceTreePanel.vue src/lib/commenterWorkspaceTree.test.ts package.json src/locales/messages.ts
git commit -m "feat(commenter): add WorkspaceTreePanel with lazy load and queue highlight"
```

---

## Task 10: `StreamContentPanel.vue`

**Files:**
- Create: `src/components/commenter/StreamContentPanel.vue`
- Create: `src/lib/commenterStreamPanel.test.ts`
- Modify: `package.json`
- Modify: `src/locales/messages.ts`

- [ ] **Step 1: Translations**

```ts
'commenter.stream.live': 'LIVE',
'commenter.stream.locked': 'LOCKED',
'commenter.stream.review': 'REVIEW',
'commenter.stream.failed': 'FAILED',
'commenter.stream.done': 'DONE',
'commenter.stream.empty': '候选已不可用（可能已回滚或清理）',
'commenter.stream.idle': 'No file selected',
```

- [ ] **Step 2: Contract test**

Create `src/lib/commenterStreamPanel.test.ts`:

```ts
import assert from 'node:assert/strict';
import fs from 'node:fs';

const source = fs.readFileSync(
  new URL('../components/commenter/StreamContentPanel.vue', import.meta.url),
  'utf8'
);

assert.match(source, /'live'/, "stream panel should reference 'live' mode");
assert.match(source, /'locked'/, "stream panel should reference 'locked' mode");
assert.match(source, /commenterApi\.getCandidateText/, 'stream panel should fetch candidate text on demand');
assert.match(source, /error_message/, 'stream panel should render error_message');

console.log('commenter stream panel PASSED');
```

Append to smoke:

```
&& tsx src/lib/commenterStreamPanel.test.ts
```

- [ ] **Step 3: Run smoke to verify failure**

```bash
pnpm smoke
```

Expected: FAIL with file-not-found.

- [ ] **Step 4: Implement the component**

Create `src/components/commenter/StreamContentPanel.vue`:

```vue
<script setup lang="ts">
import { computed, ref, watch } from 'vue';

import { commenterApi } from '../../lib/tauri';
import type { CommenterJobStatus } from '../../lib/commenterTypes';
import { use_messages } from '../../locales/messages';

const props = defineProps<{
  mode: 'live' | 'locked';
  run_key: string | null;
  relative_path: string | null;
  live_text: string;
  status: CommenterJobStatus | 'streaming' | 'idle';
  error_message: string | null;
}>();

const { t } = use_messages();

const fallback_text = ref('');
const fetching = ref(false);
const fetch_error = ref<string | null>(null);

const display_text = computed(() => (props.live_text ? props.live_text : fallback_text.value));

const badge_label = computed(() => {
  if (!props.relative_path) return '';
  if (props.status === 'failed') return t('commenter.stream.failed');
  if (props.status === 'review_needed') return t('commenter.stream.review');
  if (props.status === 'done') return t('commenter.stream.done');
  if (props.mode === 'live') return t('commenter.stream.live');
  return t('commenter.stream.locked');
});

const badge_class = computed(() => {
  if (props.status === 'failed') return 'stream-badge stream-badge--failed';
  if (props.status === 'review_needed') return 'stream-badge stream-badge--review';
  if (props.status === 'done') return 'stream-badge stream-badge--done';
  if (props.mode === 'live') return 'stream-badge stream-badge--live';
  return 'stream-badge stream-badge--locked';
});

async function maybeFetchCandidate() {
  fallback_text.value = '';
  fetch_error.value = null;
  if (props.mode !== 'locked') return;
  if (!props.run_key || !props.relative_path) return;
  if (props.live_text.length > 0) return;
  if (props.status !== 'done' && props.status !== 'review_needed') return;

  fetching.value = true;
  try {
    fallback_text.value = await commenterApi.getCandidateText(
      props.run_key,
      props.relative_path
    );
  } catch (error) {
    fetch_error.value = error instanceof Error ? error.message : String(error);
  } finally {
    fetching.value = false;
  }
}

watch(
  () => [props.mode, props.run_key, props.relative_path, props.status, props.live_text.length] as const,
  () => {
    void maybeFetchCandidate();
  },
  { immediate: true }
);
</script>

<template>
  <section class="stream-content-panel">
    <header class="stream-header">
      <span :class="badge_class">{{ badge_label }}</span>
      <span class="stream-path">{{ relative_path ?? t('commenter.stream.idle') }}</span>
    </header>

    <div
      v-if="error_message"
      class="stream-error"
    >
      {{ error_message }}
    </div>

    <pre
      v-if="display_text || fetching"
      class="stream-body"
    ><code>{{ display_text }}<span v-if="mode === 'live' && status === 'streaming'" class="stream-cursor">▍</span></code></pre>

    <div
      v-else-if="fetch_error"
      class="stream-error"
    >
      {{ fetch_error }}
    </div>

    <div
      v-else
      class="empty-state"
    >
      {{ relative_path ? t('commenter.stream.empty') : t('commenter.stream.idle') }}
    </div>
  </section>
</template>

<style scoped>
.stream-content-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.32);
}

.stream-header {
  display: flex;
  gap: 12px;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  font-family: ui-monospace, Menlo, Consolas, monospace;
  font-size: 12px;
}

.stream-badge {
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 600;
}

.stream-badge--live {
  background: rgba(99, 102, 241, 0.3);
}
.stream-badge--locked {
  background: rgba(148, 163, 184, 0.3);
}
.stream-badge--done {
  background: rgba(34, 197, 94, 0.25);
}
.stream-badge--review {
  background: rgba(245, 158, 11, 0.25);
}
.stream-badge--failed {
  background: rgba(231, 111, 111, 0.3);
}

.stream-body {
  flex: 1;
  margin: 0;
  padding: 12px;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-word;
  font-family: ui-monospace, Menlo, Consolas, monospace;
  font-size: 12px;
  line-height: 1.5;
  color: #d9ece9;
}

.stream-cursor {
  display: inline-block;
  width: 6px;
  background: currentColor;
  animation: stream-blink 1s step-start infinite;
}

@keyframes stream-blink {
  50% {
    opacity: 0;
  }
}

.stream-error {
  padding: 12px;
  background: rgba(231, 111, 111, 0.12);
  color: #f87171;
}
</style>
```

- [ ] **Step 5: Run smoke and check**

```bash
pnpm smoke
pnpm check
```

Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add src/components/commenter/StreamContentPanel.vue src/lib/commenterStreamPanel.test.ts package.json src/locales/messages.ts
git commit -m "feat(commenter): add StreamContentPanel for live/locked AI text"
```

---

## Task 11: Rewrite `RunDetailPanel.vue` as a thin tree+stream container

**Files:**
- Modify (rewrite): `src/components/commenter/RunDetailPanel.vue`

- [ ] **Step 1: Replace the file contents**

Replace `src/components/commenter/RunDetailPanel.vue` with:

```vue
<script setup lang="ts">
import { computed, ref, watch } from 'vue';

import { commenterStore } from '../../lib/commenterStore';
import { streamSliceKey } from '../../lib/commenterStreamSlice';
import StreamContentPanel from './StreamContentPanel.vue';
import WorkspaceTreePanel from './WorkspaceTreePanel.vue';

const props = defineProps<{ external_selected_file: string | null }>();

const detail = computed(() => commenterStore.state.selected_run_detail);
const run = computed(() => detail.value?.run ?? null);

const follow_mode = ref<'live' | 'locked'>('live');
const selected_file = ref<string | null>(null);

watch(
  () => run.value?.current_file ?? null,
  (current) => {
    if (follow_mode.value === 'live' && current) {
      selected_file.value = current;
    }
  },
  { immediate: true }
);

watch(
  () => props.external_selected_file,
  (path) => {
    if (path) {
      follow_mode.value = 'locked';
      selected_file.value = path;
    }
  }
);

function onTreeSelect(relative_path: string) {
  follow_mode.value = 'locked';
  selected_file.value = relative_path;
}

const live_slice = computed(() => {
  if (!run.value || !selected_file.value) return null;
  const key = streamSliceKey(run.value.run_key, selected_file.value);
  return commenterStore.state.live_streams.get(key) ?? null;
});

const job_status = computed(() => {
  if (!detail.value || !selected_file.value) return 'idle' as const;
  const job = detail.value.jobs.find((j) => j.relative_path === selected_file.value);
  if (!job) return 'idle' as const;
  if (live_slice.value?.status === 'streaming') return 'streaming' as const;
  return job.status;
});

const error_message = computed(() => {
  if (!detail.value || !selected_file.value) return null;
  const job = detail.value.jobs.find((j) => j.relative_path === selected_file.value);
  return job?.error_message ?? live_slice.value?.error ?? null;
});
</script>

<template>
  <div class="run-detail-grid">
    <WorkspaceTreePanel @select-file="onTreeSelect" />
    <StreamContentPanel
      :mode="follow_mode"
      :run_key="run?.run_key ?? null"
      :relative_path="selected_file"
      :live_text="live_slice?.text ?? ''"
      :status="job_status"
      :error_message="error_message"
    />
  </div>
</template>

<style scoped>
.run-detail-grid {
  display: grid;
  grid-template-columns: 280px 1fr;
  gap: 12px;
  min-height: 420px;
}
</style>
```

- [ ] **Step 2: Verify smoke + types**

```bash
pnpm check
```

Expected: vue-tsc clean, smoke green.

- [ ] **Step 3: Commit**

```bash
git add src/components/commenter/RunDetailPanel.vue
git commit -m "refactor(commenter): rewrite RunDetailPanel as tree+stream container"
```

---

## Task 12: Wire the new layout into `CommentOrchestratorPage.vue`

**Files:**
- Modify: `src/pages/CommentOrchestratorPage.vue`

- [ ] **Step 1: Replace the runtime workspace section**

In `src/pages/CommentOrchestratorPage.vue`:

Add to the imports near the top of `<script setup>`:

```ts
import RunHeaderStrip from '../components/commenter/RunHeaderStrip.vue';
```

Add two refs in the script (place next to `active_run_tab`):

```ts
const external_selected_file = ref<string | null>(null);

function onLogSelectFile(path: string) {
  external_selected_file.value = path;
  active_run_tab.value = 'detail';
}
```

Replace the existing `<section class="settings-section-card settings-section-card--subtle settings-run-workspace">…</section>` block with:

```vue
<section class="settings-section-card settings-section-card--subtle settings-run-workspace">
  <div class="page-intro settings-runtime-intro">
    <h3>{{ t('settings.workspace.runtime') }}</h3>
    <p>{{ t('settings.workspace.runtimeHelp') }}</p>
    <p class="settings-runtime-note">
      {{ t('settings.workspace.persistenceHelp') }}
    </p>
  </div>

  <RunHeaderStrip />

  <div class="runtime-grid">
    <ExecutionLogPanel @select-file="onLogSelectFile" />

    <div class="runtime-tab-area">
      <div
        class="workspace-tablist"
        role="tablist"
        :aria-label="t('settings.workspace.runtime')"
      >
        <button
          v-for="tab in run_tabs"
          :key="tab.key"
          class="workspace-tab"
          :class="{ active: active_run_tab === tab.key }"
          role="tab"
          :aria-selected="active_run_tab === tab.key"
          :tabindex="active_run_tab === tab.key ? 0 : -1"
          @click="active_run_tab = tab.key"
        >
          <span>{{ t(tab.label_key) }}</span>
          <span
            v-if="tab.count > 0"
            class="workspace-tab-count"
          >
            {{ tab.count }}
          </span>
        </button>
      </div>

      <QueueRunsTable v-if="active_run_tab === 'queue'" />
      <RunDetailPanel
        v-else-if="active_run_tab === 'detail'"
        :external-selected-file="external_selected_file"
      />
      <ReviewJobsPanel v-else-if="active_run_tab === 'review'" />
      <RunHistoryPanel v-else />
    </div>
  </div>
</section>
```

Add to the `<style>` of the page (or, if scoped styles are not used here, the project's styles.css):

```css
.runtime-grid {
  display: grid;
  grid-template-columns: 240px 1fr;
  gap: 12px;
  min-height: 480px;
}

.runtime-tab-area {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-width: 0;
}
```

Make sure the existing `import ExecutionLogPanel from '../components/commenter/ExecutionLogPanel.vue';` is present at the top — add it if not.

- [ ] **Step 2: Run the full verification**

```bash
pnpm check
cargo test commenter --manifest-path src-tauri/Cargo.toml
```

Both must pass.

- [ ] **Step 3: Manual smoke (mark each item)**

Start the dev app via `pnpm tauri:dev`. Verify:

- [ ] Click Start on a queued run → `RunHeaderStrip` flips to `running` within 1 s
- [ ] `ExecutionLogPanel` shows a row per file as jobs progress; chevron expands the phase timeline
- [ ] Detail tab shows the workspace tree with the project structure; queued files are at full opacity, others muted
- [ ] When a file enters `requesting`, the right `StreamContentPanel` shows `LIVE` and starts populating text
- [ ] Click a different file in the tree → badge flips to `LOCKED`; right panel shows that file's content (live or fetched candidate)
- [ ] Click a row in `ExecutionLogPanel` → the page jumps to Detail tab and the right panel locks to that file
- [ ] Force a credential failure (clear `OPENAI_API_KEY` and any direct token) → file row turns red; right panel shows `FAILED` badge and error
- [ ] Switch to a different run → log entries change; live stream text from the previous run is gone

- [ ] **Step 4: Commit**

```bash
git add src/pages/CommentOrchestratorPage.vue
git commit -m "feat(commenter): wire execution log, run header, and tree+stream into orchestrator page"
```

---

## Self-review summary

After Task 12, the spec sections map to tasks as follows:

| Spec section | Tasks |
| --- | --- |
| Architecture (file map, layout) | 7, 8, 9, 10, 11, 12 |
| Components (RunHeaderStrip, ExecutionLogPanel, WorkspaceTreePanel, StreamContentPanel, RunDetailPanel, page) | 7, 8, 9, 10, 11, 12 |
| Data flow → `buildFileLogEntries` | 1 |
| Data flow → `live_streams` slice + LRU + cap | 2, 6 |
| Data flow → workspace tree lazy load | 3, 5, 9 |
| Data flow → Tauri commands (`commenter_list_dir`, `commenter_get_candidate_text`) | 3, 4, 5 |
| Error handling → tree errors, run not found, missing artifact, single-file 5 MB cap, cross-run pollution | 2, 3, 4, 9, 10, 11 |
| Testing → pure logic units | 1, 2 |
| Testing → contract tests for components | 7, 8, 9, 10 |
| Testing → Rust command tests | 3, 4 |
| Testing → manual smoke list | 12 |

If during execution you find a spec requirement with no implementing task, stop and add the task before continuing.
