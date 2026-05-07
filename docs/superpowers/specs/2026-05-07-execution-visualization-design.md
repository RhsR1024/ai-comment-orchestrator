# Execution Visualization for Comment Orchestrator

> Surface live run state — execution log, workspace tree, and AI streaming — inside the Comment Orchestrator runtime workspace, replacing the current opaque "click Start and hope" experience.

## Problem

Clicking **Start** in `CommentOrchestratorPage` flips a status badge but offers no live feedback: users cannot see which file is being processed, whether the request was sent, the AI's streaming response, or per-file success/failure beyond a numeric counter. The backend already emits sixteen event kinds (`RequestStarted`, `StreamChunk`, `JobUpdated`, …) on `commenter://state`, and `commenterStore.execution_logs` already buffers them — the gap is purely on the UI side: a flat `ExecutionLogPanel` that is not even mounted from the page, plus a Detail tab that shows only metric cards and the last six events.

## Design Decisions

| Decision | Choice | Reason |
| --- | --- | --- |
| Where the execution log lives | Page-level inside the orchestrator runtime workspace (not global, not Detail-tab-only) | Visible across all four tabs; does not pollute global navigation. |
| Stream-panel lifecycle | Follow current file by default; click-to-lock | Combines live monitoring with post-run review without forcing tab navigation. |
| Log granularity | Compact — one entry per file with expandable phase history | Answers "did the request go out?" at a glance; detail is one click away. |
| Workspace-tree scope | Full workspace source tree, queued files highlighted, others muted | Provides project-structure context while distinguishing what is in the run. |
| Delivery | Big-bang single redesign | User explicitly chose a coherent end-to-end ship over phased rollout. |

## Architecture

Only the runtime-workspace card inside `src/pages/CommentOrchestratorPage.vue` changes. Everything above it (page intro, profile panel, summary cards, Diff-tool advanced settings) is untouched.

New runtime-workspace structure (top to bottom):

1. **`RunHeaderStrip`** — run name, status badge, progress bar, success / review / fail counts, Pause / Resume / Cancel. Visible across all four tabs.
2. **Two-column body** (CSS grid, `240px 1fr`):
   - Left: **`ExecutionLogPanel`** (rewritten — compact per-file rows with expandable phase timeline)
   - Right: tab strip (Queue / Detail / Review / History) + the active tab's content
3. **Detail tab body** (CSS grid, `280px 1fr`):
   - Middle: **`WorkspaceTreePanel`** (new — lazy-loaded source tree with queue highlighting and current-file auto-expand)
   - Right: **`StreamContentPanel`** (new — live AI text accumulator with click-to-lock)

### File map

| Path | Action |
| --- | --- |
| `src/components/commenter/RunHeaderStrip.vue` | New |
| `src/components/commenter/ExecutionLogPanel.vue` | Rewrite |
| `src/components/commenter/WorkspaceTreePanel.vue` | New |
| `src/components/commenter/StreamContentPanel.vue` | New |
| `src/components/commenter/RunDetailPanel.vue` | Rewrite (thin container for tree + stream) |
| `src/pages/CommentOrchestratorPage.vue` | Modify (new runtime-workspace layout) |
| `src/lib/commenterFileLog.ts` | New (pure aggregation function) |
| `src/lib/commenterStore.ts` | Extend with `live_streams` slice + LRU eviction |
| `src-tauri/src/commenter/commands.rs` | Add `commenter_list_dir` and `commenter_get_candidate_text` |
| `src/lib/tauri.ts` | Expose the two new commands |
| `package.json` | Append the new test scripts to `smoke` |

### Reused infrastructure

- `commenter://state` event channel — no contract changes
- `commenterStore.execution_logs` ring buffer (cap 400) — kept as raw stream; new UI derives from it
- Optimistic status flip + synthetic `run_started` event already in `commenterStore.startRun` — provides sub-second feedback; new UI surfaces it
- View helpers in `commenterView.ts` (`run_status_label`, `is_run_finished`, `event_message`)
- Project profile's `root_path` — workspace-tree root

## Components

### `RunHeaderStrip.vue`

- **Inputs**: `commenterStore.state.selected_run_detail.run`
- **Renders**: `run_key`, status badge (color by status), progress bar, `succeeded / total · review_needed · failed`, control buttons
- **Button visibility by status**: `running` → Pause + Cancel; `paused` → Resume + Cancel; terminal states (`completed*`, `cancelled`, `failed`) → no controls; absent run → entire strip hidden
- Purely run-level chrome — no control over streams or selection

### `ExecutionLogPanel.vue` (rewrite)

- **Inputs**: `selected_run_detail.jobs`, `selected_run_detail.events`, `selected_run_key`
- **Internal**: per-file aggregation via `buildFileLogEntries(jobs, events)` (computed)
- **Renders**: top sticky row (run-level status), then a list of `FileLogEntry` rows; each row shows phase icon, filename, elapsed time, current short phrase
- **Interaction**: chevron click toggles phase-timeline expansion (state held by `Set<relative_path>`); row click emits `select-file` (page routes it to Detail tab)
- **Cap**: render first 200 entries; older summarized as `… N more`
- **Empty state**: when no run is selected, shows `t('commenter.idle')`

### `WorkspaceTreePanel.vue` (new)

- **Inputs**: `profile_key: string`, `queued_paths: Set<string>`, `current_file: string | null`, `file_status_map: Map<path, JobStatus>`
- **Local state**: `tree: TreeNode` reactive root; `expanded: Set<path>`
- **Lazy load**: on directory expand, call `commenter_list_dir(profile_key, relative_path)`; cache children on the node; do not refetch unless user collapses and reopens
- The component does not need to know the absolute `root_path`; the Tauri command resolves `profile_key` → `root_path` server-side
- **Auto-expand**: watcher on `current_file` ensures all ancestor directories are expanded
- **Visual rules**: queued files at full opacity with status icon; non-queued files at 50% opacity with no status icon; click only registers on queued files
- **Emits**: `select-file(relative_path)` on queued-file click

### `StreamContentPanel.vue` (new)

- **Inputs**: `mode: 'live' | 'locked'`, `relative_path`, `live_text`, `final_text`, `status`, `error_message`
- **Renders**: top bar (LIVE / LOCKED / REVIEW / FAILED / DONE badge + path + follow indicator), main monospaced text area (auto-scroll to bottom while in live mode), bottom bar (byte count, elapsed)
- **Stateless on mode**: parent decides mode and provides text; component only paints
- **Failure rendering**: when `status === 'failed'`, shows `error_message` in a red panel and stops the cursor blink

### `RunDetailPanel.vue` (rewrite, thin)

- Holds two local refs: `selected_file` and `follow_mode: 'live' | 'locked'`
- Watches `selected_run_detail.run.current_file`: when `follow_mode === 'live'`, syncs `selected_file` to it
- Renders `<WorkspaceTreePanel>` + `<StreamContentPanel>` side by side
- `WorkspaceTreePanel`'s `select-file` → set `follow_mode = 'locked'`, `selected_file = path`
- Accepts a parent prop `external_selected_file` (set by the page when the user clicks a row in `ExecutionLogPanel`) → same effect as a tree click

### `CommentOrchestratorPage.vue` (modify)

```
<page intro / profile cards / summary cards (unchanged) />

<section class="runtime-workspace">
  <RunHeaderStrip />
  <div class="runtime-grid">
    <ExecutionLogPanel @select-file="onLogSelectFile" />
    <div class="runtime-tab-area">
      <Tabs />
      <QueueRunsTable      v-if="active === 'queue'"   />
      <RunDetailPanel      v-else-if="active === 'detail'"   :external-selected-file="external_selected_file" />
      <ReviewJobsPanel     v-else-if="active === 'review'"   />
      <RunHistoryPanel     v-else />
    </div>
  </div>
</section>
```

`onLogSelectFile(path)` sets `active = 'detail'` and `external_selected_file = path`.

## Data Flow & State

### Per-file aggregation: `src/lib/commenterFileLog.ts`

```ts
export type PhaseTag =
  | 'queued' | 'requested' | 'first_chunk' | 'response_done'
  | 'validated' | 'written' | 'review_requested' | 'failed' | 'rolled_back';

export interface FileLogEntry {
  relative_path: string;
  status: CommenterJobStatus;
  phases: { phase: PhaseTag; at: number; level: CommenterEventLevel }[];
  started_at: number | null;
  ended_at: number | null;
  error_message: string | null;
}

export function buildFileLogEntries(
  jobs: CommenterJobRecord[],
  events: CommenterEventPayload[]
): FileLogEntry[];
```

- `jobs` is the authoritative source for `status`; events provide the phase timeline
- Events are grouped by `relative_path`; entries are returned in `jobs` order
- Orphan events (path not in `jobs`) are silently dropped
- Pure function, no side effects, no Vue reactivity — testable in isolation

### Live streaming: `commenterStore.live_streams`

```ts
interface LiveStreamSlice {
  text: string;
  started_at: number;
  last_chunk_at: number;
  status: 'streaming' | 'completed' | 'failed';
  error: string | null;
}

state.live_streams: Map<string /* `${run_key}|${relative_path}` */, LiveStreamSlice>;
```

Event-to-slice mapping (added inside `appendExecutionEvent`):

| Event kind | Operation |
| --- | --- |
| `request_started` | Create new slice with empty `text` and `status='streaming'` |
| `stream_chunk` | Append `event.message` to `text`; update `last_chunk_at` |
| `model_response_completed` | Set `status='completed'` |
| `job_failed` | Set `status='failed'` and `error=event.message` |

**Memory bounds**:
- LRU by `last_chunk_at`: keep `text` for at most 30 files; older slices retain metadata but `text` is cleared
- Per-slice `text` hard cap of 5 MB; further chunks ignored after appending a `[... truncated, full text in candidate.txt]` marker
- `commenterStore.selectRun` clears `live_streams` entirely (streams are run-scoped working memory)

### Workspace tree

- `WorkspaceTreePanel` owns a local `TreeNode` reactive — not in the store
- Lazy load on directory expand via `commenter_list_dir`; children cached on the node
- Tree never exceeds the displayed subset; large projects pay only for what users open

### New Tauri commands

| Command | Signature | Behavior |
| --- | --- | --- |
| `commenter_list_dir` | `(profile_key: String, relative_path: String) -> Result<Vec<DirEntry>, String>` | Looks up the profile by `profile_key`, then reads one level under `profile.root_path/relative_path`. Filters entries against `profile.exclude_directories`. Canonicalizes the resolved path; refuses if it escapes `root_path`. Returns `Vec<{ name, kind: 'dir' \| 'file', relative_path }>`. Returns `Err` if `profile_key` is unknown. |
| `commenter_get_candidate_text` | `(run_key: String, relative_path: String) -> Result<String, String>` | Resolves to `<data_root>/commenter/runs/<run_key>/candidates/<relative_path>.candidate` (matches existing `artifact_output_path(...".candidate")` layout). Returns `Ok("")` when the artifact does not exist (so the UI shows a placeholder rather than an error). Other IO errors surface as `Err`. |

Both commands are read-only and do not touch SQLite.

### Lifecycle scenarios

- **Cold open / page entry** — Existing `commenterStore.initialize()` already subscribes to events and loads the selected run's detail. `ExecutionLogPanel` re-derives per-file state from historical events. `live_streams` starts empty; locked clicks on completed files lazy-fetch via `commenter_get_candidate_text`.
- **User clicks Start** — Store optimistically flips `run.status='running'` and emits a synthetic `run_started` event (already in code). `RunHeaderStrip` flips immediately. Subsequent `request_started` / `stream_chunk` / `job_updated` events arrive on the same channel and populate the log and stream panel.
- **Run completes** — Status flips to `completed*`/`cancelled`/`failed`; control buttons disappear; `live_streams` retained until the user navigates away or LRU evicts.
- **App restart** — Events and jobs are persisted in SQLite; UI re-hydrates from `getRunDetail`. `live_streams` starts empty by design.

## Error Handling

| Failure | Source signal | UI response |
| --- | --- | --- |
| Missing credentials / auth failure | `job_failed` with `level=error` | Red row in `ExecutionLogPanel`; `⚠ FAILED` badge + error body in `StreamContentPanel` |
| SSE stream cut off mid-flight | `job_failed` | Same; partial text retained in `live_streams[file].text` |
| Validation rejection (markdown fence, severe shrink, language anomaly) | `job_updated.status='review_needed'` or `'failed'` | Yellow ⚠ badge for review; red ✗ for failed |
| Whole run fails | `run_completed` with `status='failed'` | `RunHeaderStrip` badge red; control buttons hidden |
| `commenter_list_dir` returns Err (missing dir, permission denied, path traversal) | Tauri Err string | Tree node shows red `!` with tooltip; click-to-retry; never crashes UI |
| Profile root_path entirely unreachable | `commenter_list_dir` Err on root | `WorkspaceTreePanel` replaced with empty state pointing to Profile config |
| `commenter_get_candidate_text` returns `""` | Empty string | Placeholder: `候选已不可用（可能已回滚或清理）` |
| `commenter_get_candidate_text` returns Err | Tauri Err string | In-panel error banner; other files unaffected |
| Event subscription drops | `commenterStore.error_message` set | `RunHeaderStrip` badge appended with `⚠ Live updates lost — refresh`; click triggers `commenterStore.refresh()`; no automatic reconnect |
| Selected run no longer exists | `getRunDetail` Err | Store clears `selected_run_key`; subordinate panels show empty states; queue list still refreshes |
| Locked-click on LRU-evicted file | `live_text === ''` and `status === 'completed'` | `StreamContentPanel` auto-fetches via `commenter_get_candidate_text`; spinner during fetch |
| Cross-run event pollution | Events arrive for non-selected run | Per-file derivations filter by `selected_run_key`; `live_streams` keys include `run_key` so naturally isolated |
| Single-file stream exceeds 5 MB | Internal cap | Stop appending; insert truncation marker; on-disk candidate remains complete |

**Out of scope (YAGNI)**:
- No automatic reconnect or offline event buffering
- No log export (review/history flows already cover post-mortem)
- No UI-layer retry — `retryJob` already exists and is surfaced via the review flow
- No persistence of stream text to IndexedDB — artifacts on disk are source of truth

## Testing

Test stack: `tsx + node:assert/strict` scripts wired into `pnpm smoke`; backend uses `cargo test`. New script files must be appended to `package.json`'s `smoke` script.

### Pure logic unit tests

**`src/lib/commenterFileLog.test.ts`** (new — real unit tests, not source-text matching):
- Empty `jobs` and empty `events` → `[]`
- Single file with full phase sequence → entry phases ordered by timestamp
- Interleaved events for multiple files → grouped correctly by `relative_path`
- `job_failed` for a path → `entry.error_message` set to the most recent failure message
- Orphan event (path absent from `jobs`) → silently dropped, no error

**`src/lib/commenterStreamSlice.test.ts`** (new):
- `request_started` creates an empty slice
- Multiple `stream_chunk` events concatenate in order
- `model_response_completed` flips `status` without touching `text`
- 31st streaming file evicts the slice with the oldest `last_chunk_at` (text cleared, metadata kept)
- Single slice exceeding 5 MB stops appending and inserts the truncation marker

### Contract-style component tests

Match the existing `commenterExecutionLog.test.ts` pattern (load source as string, run `assert.match(...)` to confirm a stable contract).

- **`src/lib/commenterRunHeader.test.ts`** (new) — `RunHeaderStrip.vue` source contains `pauseRun`, `cancelRun`, `resumeRun` calls and renders `progress`/`current_file`
- **`src/lib/commenterWorkspaceTree.test.ts`** (new) — `WorkspaceTreePanel.vue` invokes `commenter_list_dir`, references `current_file` for auto-expand, and uses `queued_paths` for highlighting
- **`src/lib/commenterStreamPanel.test.ts`** (new) — `StreamContentPanel.vue` contains both `'live'` and `'locked'` literals and invokes `commenter_get_candidate_text`
- **`src/lib/commenterExecutionLog.test.ts`** (update) — drop the flat-log assertions; add assertions that the panel calls `buildFileLogEntries` and supports an expansion toggle

### Rust command tests

In `src-tauri/src/commenter/commands.rs::tests`:

- `list_dir_returns_entries_under_profile_root` — temp profile with mixed dirs/files and an excluded subdir; command returns correct `kind` per entry and excludes the configured directory
- `list_dir_rejects_path_outside_profile_root` — `relative_path = "../etc"` or absolute path returns `Err`
- `get_candidate_text_returns_artifact_content` — pre-write `<artifacts>/<run>/<file>.candidate.txt`; command returns the same string
- `get_candidate_text_returns_empty_when_artifact_missing` — non-existent artifact returns `Ok("")` (never `Err`)

### Verification commands

- Frontend: `pnpm check` (runs `vue-tsc --noEmit && pnpm smoke`)
- Backend: `cargo test commenter --manifest-path src-tauri/Cargo.toml`

### Manual smoke checklist

- Click Start on a queued run → `RunHeaderStrip` flips to `running` within 1 second
- Stream text scrolls smoothly without jitter
- Tree expansion remains responsive on a 1000+ file project
- Switching to a different run clears the previous run's stream text immediately
- Pause → Resume on the same run continues stream accumulation rather than resetting
- Forcing an SSE failure (e.g., wrong API key) shows `⚠ FAILED` badge and error body in `StreamContentPanel`

### Excluded from automated tests

- `RunDetailPanel.vue` (thin container — would test only assembly)
- CSS / pixel layout
- Full Tauri runtime simulation for E2E (infrastructure cost outweighs benefit; manual smoke covers the gap)
