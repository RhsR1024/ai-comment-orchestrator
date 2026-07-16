# State Management

> How state is managed in this project.

---

## Overview

<!--
Document your project's state management conventions here.

Questions to answer:
- What state management solution do you use?
- How is local vs global state decided?
- How do you handle server state?
- What are the patterns for derived state?
-->

(To be filled by the team)

---

## State Categories

<!-- Local state, global state, server state, URL state -->

(To be filled by the team)

---

## When to Use Global State

<!-- Criteria for promoting state to global -->

(To be filled by the team)

---

## Server State

<!-- How server data is cached and synchronized -->

### Preserve Live Request Events Across Detail Refreshes

**Problem**: While a file is still streaming, the backend emits `request_started`, `stream_chunk`, and `model_response_completed` over `commenter://state`, but the selected run detail snapshot may not contain those per-request events until the worker finishes and persists the job outcome. If the UI replaces `selected_run_detail.events` with the stale snapshot on every refresh, the request detail tab can show `尚未发起请求` while text is actively streaming.

**Required Store Contract**:
```typescript
mergeRunDetailEvents(
  nextDetail: CommenterRunDetail,
  currentDetail: CommenterRunDetail | null,
  executionLogs: CommenterEventPayload[]
): CommenterEventPayload[]
```

The merge must:
- keep non-`stream_chunk` events from the next backend detail
- preserve non-`stream_chunk` events already observed in the current selected detail for the same `run_key`
- preserve matching events from `state.execution_logs`
- ignore events for other runs
- deduplicate by `kind`, `run_key`, `relative_path`, `created_at`, and `message`
- return events in chronological order for detail timelines

`applySelectedRunDetail()` must also keep newer live stream slices for the selected run when a refresh rebuilds slices from a stale backend detail.
When a refreshed run is terminal, it must reconcile any legacy live slice still marked `streaming`: failed jobs become failed slices and every other stale active slice becomes completed. This fallback handles older persisted CodeBuddy events that omitted the stream anchor from their terminal event.

High-frequency stream updates use this batch boundary:

```typescript
applyEventsToStreamSlices(
  current: Map<string, LiveStreamSlice>,
  events: CommenterEventPayload[]
): Map<string, LiveStreamSlice>
```

It clones the stream map at most once per animation-frame batch and preserves event order. Active-run polling requests only `listRuns`, `listReviewJobs`, and the selected `getRunDetail`, with one shared in-flight promise. A slow poll must not overlap the next 3-second tick; manual refresh still loads profiles/settings.

Live stream text is a preview cache, not durable history. It retains at most four file slices with text and at most 512 KiB per slice. Backend detail polling contains structural events only; full completed text is fetched from candidate/original artifacts on demand.

The presentation layer may reveal `LiveStreamSlice.text` progressively, but the store must always ingest complete chunk payloads immediately. Typewriter timers are component-local and must not be stored, persisted, or used to decide whether a job is complete. Completion comes only from structural backend state/events.

**Component Contract**: `StreamContentPanel.vue` receives `streamLastChunkAt` from the selected `LiveStreamSlice`. If a file is in `streaming` status or has live text but no completed response event yet, the request detail summary and timeline must show `commenter.request.streaming` / `commenter.event.stream_chunk` instead of the idle request state.

**Tests Required**:
- `src/lib/commenterStoreEvents.test.ts` covers merging stale backend detail with live request lifecycle events.
- `src/lib/commenterStreamPanel.test.ts` asserts streaming request detail labels and live chunk timing remain wired.
- `src/lib/commenterStreamSlice.test.ts` covers batched ordering and terminal `completed`/`failed` transitions.

**Validation matrix**:

| Case | Expected behavior |
| --- | --- |
| Multiple chunks arrive in one animation frame | One map clone; text concatenates in event order |
| Runtime refresh exceeds the polling interval | Later ticks reuse the in-flight promise; detail requests do not overlap |
| Backend emits `job_failed` after stream timeout | Slice becomes `failed` and shows the error instead of `streaming` |

- Good: live events update immediately while one lightweight poll recovers missed events.
- Base: completion changes the slice to `completed` without changing accumulated text.
- Bad: every delta clones the full map, or a timer starts overlapping full refreshes.

### Reuse Initialized Workspace State And Bound Directory Metadata

Route navigation remounts `CommentOrchestratorPage.vue`, but the module-level commenter store survives. `initialize()` must therefore use one shared in-flight promise and become a no-op after the first successful full refresh. Explicit `refresh()` and mutations still refresh server state.

The run tree remains lazy. It caches only directory listings requested through `listDir`, never file contents, and uses a 128-directory LRU bound. Cache keys include the profile root and `updated_at`, so saving a profile cannot reuse listings from an old root/filter configuration. A manual root retry invalidates the selected profile's directory entries before reading disk again.

```typescript
let initialization_pending: Promise<void> | null = null;
let initialized = false;
const MAX_CACHED_DIRECTORIES = 128;
```

**Test points**:

- `src/lib/commenterApiShape.test.ts` asserts initialization reuse and in-flight deduplication remain present.
- `src/lib/commenterWorkspaceTree.test.ts` asserts lazy directory reuse, the fixed bound, and manual invalidation.

**Forbidden**: recursively preload a configured project into a reactive tree during route entry, or repeat the full profiles/settings/runs initialization on every settings/workspace navigation.

---

## Common Mistakes

<!-- State management mistakes your team has made -->

(To be filled by the team)
