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

**Component Contract**: `StreamContentPanel.vue` receives `streamLastChunkAt` from the selected `LiveStreamSlice`. If a file is in `streaming` status or has live text but no completed response event yet, the request detail summary and timeline must show `commenter.request.streaming` / `commenter.event.stream_chunk` instead of the idle request state.

**Tests Required**:
- `src/lib/commenterStoreEvents.test.ts` covers merging stale backend detail with live request lifecycle events.
- `src/lib/commenterStreamPanel.test.ts` asserts streaming request detail labels and live chunk timing remain wired.

---

## Common Mistakes

<!-- State management mistakes your team has made -->

(To be filled by the team)
