# Settings Workspace Cleanup And Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove obsolete Tools-era frontend leftovers and add one small round of structure polish to the settings workspace.

**Architecture:** Add a source-level smoke test that locks the cleanup contract, then remove unused tool pages and locale keys. After that, add lightweight summary/grouping affordances to `CommentOrchestratorPage.vue` and the shared stylesheet without changing commenter backend behavior.

**Tech Stack:** Vue 3, TypeScript, Node fs-based smoke tests via `tsx`, existing reactive commenter store

---

## File Structure
- Create: `src/lib/settingsWorkspaceCleanup.test.ts`
- Modify: `package.json`
- Modify: `src/pages/CommentOrchestratorPage.vue`
- Modify: `src/styles.css`
- Modify: `src/locales/messages.ts`
- Delete: `src/pages/ToolsHubPage.vue`
- Delete: `src/pages/ToolPlaceholderPage.vue`

### Task 1: Lock cleanup and polish targets with a failing smoke test
- [ ] Add a test asserting obsolete tool pages are gone, obsolete tool locale keys are gone, and the settings workspace exposes explicit summary/grouping markup.
- [ ] Add the test to the smoke script.
- [ ] Run the test and verify it fails before implementation.

### Task 2: Remove obsolete Tools leftovers
- [ ] Delete the unused tool page files.
- [ ] Remove obsolete tool-related locale keys and unused styles.

### Task 3: Add lightweight structure polish
- [ ] Add summary cards and clearer section grouping around project config, advanced settings, and run workspace.
- [ ] Add only the CSS needed to support the clearer hierarchy.
- [ ] Run smoke and build to verify no regressions.
