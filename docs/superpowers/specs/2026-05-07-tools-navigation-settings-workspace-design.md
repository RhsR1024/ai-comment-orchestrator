# Tools Navigation Settings Workspace Design

**Date:** 2026-05-07

## Goal
Remove the top-level Tools information architecture and make Settings the single entry point for the ai-comment-orchestrator workspace, while reducing density so the page is easier to use during functional bring-up.

## Decisions
- Remove the Tools level from the sidebar.
- Route `/` directly to `/settings`.
- Route `/settings` to the ai-comment-orchestrator workspace.
- Keep `/tools` and `/tools/comment-orchestrator` as compatibility redirects to `/settings`.
- Replace the current two-column workspace with a single-column settings workbench.
- Keep Diff Tool Settings inside a collapsed Advanced Settings section.
- Replace the stacked run panels with tabs for Queue, Detail, Review, and History.

## Layout
1. Compact app shell header
2. Settings page intro with one title and one short description
3. Project Profiles panel with looser field rhythm and full-width long fields
4. Advanced Settings disclosure containing Diff Tool Settings
5. Run workspace tabs:
   - Queue Runs
   - Run Detail
   - Review Queue
   - History & Rollback

## Scope
This change is intentionally limited to navigation, routing, layout density, and lightweight interaction structure. It does not change backend commenter behavior.

## Compatibility
- Existing placeholder routes for Console, Tasks, and History remain.
- Existing deep links under `/tools` continue to resolve via redirect.
