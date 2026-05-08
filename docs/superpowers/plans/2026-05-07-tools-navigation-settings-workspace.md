# Tools Navigation Settings Workspace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the ai-comment-orchestrator workspace into Settings, remove the Tools level, and reduce UI density for faster functional bring-up.

**Architecture:** Repoint router and sidebar navigation to `/settings`, preserve old `/tools` paths as redirects, then reshape the settings page into a single-column workbench with an advanced-settings disclosure and tabbed run panels. Keep existing commenter panels and store behavior; only reorganize the frontend shell and layout.

**Tech Stack:** Vue 3, TypeScript, Vue Router, lucide-vue-next, existing reactive commenter store, existing smoke tests via `tsx`

---

## File Structure
- Modify: `src/lib/commenterRoute.test.ts`
- Modify: `src/router/index.ts`
- Modify: `src/components/Sidebar.vue`
- Modify: `src/App.vue`
- Modify: `src/pages/CommentOrchestratorPage.vue`
- Modify: `src/components/commenter/ProjectProfilesPanel.vue`
- Modify: `src/components/commenter/QueueRunsTable.vue`
- Modify: `src/locales/messages.ts`
- Modify: `src/styles.css`

### Task 1: Lock the new route contract with tests
- [ ] Update `src/lib/commenterRoute.test.ts` to assert `/` resolves into `/settings`, `/settings` exists, and `/tools` plus `/tools/comment-orchestrator` redirect to `/settings`.
- [ ] Run `pnpm exec tsx src/lib/commenterRoute.test.ts` and verify it fails before route changes.
- [ ] Implement router/sidebar changes.
- [ ] Re-run `pnpm exec tsx src/lib/commenterRoute.test.ts` and verify it passes.

### Task 2: Rebuild Settings as the main workspace
- [ ] Rework `CommentOrchestratorPage.vue` into a single-column settings workbench with compact intro, advanced-settings disclosure, and run tabs.
- [ ] Relax the profile form density in `ProjectProfilesPanel.vue`, especially long fields.
- [ ] Add the minimal CSS and locale strings needed for the new structure.

### Task 3: Verify the smoke suite and typecheck
- [ ] Run `pnpm run smoke`.
- [ ] Run `pnpm run build`.
- [ ] Fix any regressions found by the checks.
