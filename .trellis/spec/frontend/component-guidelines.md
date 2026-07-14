# Component Guidelines

> How components are built in this project.

---

## Overview

<!--
Document your project's component conventions here.

Questions to answer:
- What component patterns do you use?
- How are props defined?
- How do you handle composition?
- What accessibility standards apply?
-->

(To be filled by the team)

---

## Component Structure

<!-- Standard structure of a component file -->

(To be filled by the team)

---

## Props Conventions

<!-- How props should be defined and typed -->

(To be filled by the team)

---

## Styling Patterns

<!-- How styles are applied (CSS modules, styled-components, Tailwind, etc.) -->

(To be filled by the team)

---

## Accessibility

<!-- A11y requirements and patterns -->

(To be filled by the team)

---

## Common Mistakes

### Viewport Workspaces Must Propagate Scroll Constraints

**Symptom**: Opening a long streamed file creates a scrollbar on the far-right edge of the application. Scrolling to the end moves the sidebar, run header, and detail rail out of view.

**Cause**: Declaring `overflow: auto` on the leaf content is not sufficient. Every grid or flex ancestor between the viewport shell and the scroll owner must allow shrinking; otherwise its default `min-height: auto` lets content expand the document.

**Required Pattern**:

```css
.viewport-workspace {
  height: 100vh;
  overflow: hidden;
}

.workspace-grid,
.workspace-rail,
.content-panel,
.scroll-owner {
  min-height: 0;
}

.workspace-grid,
.workspace-rail,
.content-panel {
  overflow: hidden;
}

.scroll-owner {
  overflow: auto;
  scrollbar-color: #3b4852 var(--aco-surface-1);
  scrollbar-width: thin;
}
```

The run workspace uses this chain across `.run-reference-shell`, `.run-detail-reference-grid`, `.run-left-rail` / `.run-stream-rail`, and the tree or stream content element. Responsive breakpoints may restore document scrolling when the rails stack.

**Test Points**:

- `src/lib/referenceStyleLayout.test.ts` asserts that the run shell is viewport-constrained.
- The same test asserts that the stream rail contains overflow and that both left and right scroll owners receive the dark scrollbar treatment.
- `pnpm run build` must pass to validate scoped component styles and templates.

### Run Workspace Rail Variants Must Keep Creation Entrypoints

**Symptom**: A user saves a project profile in the project configuration view, then opens the run workspace and sees an empty runs table with no way to select the profile or start work.

**Cause**: `QueueRunsTable.vue` has a compact `rail` variant inside `RunDetailPanel.vue`. If the full queue form is hidden for this variant, first-time users cannot create the first run because there is no existing run to select.

**Required Pattern**:
```vue
<QueueRunsTable variant="rail" />
```

The rail variant must still expose:
- `form.profile_key` project selection from `commenterStore.state.profiles`
- run defaults from the selected profile
- a primary action that enqueues a run and sends the start command
- an empty-profile state linking back to `/settings`

**Test Point**: `src/lib/referenceStyleLayout.test.ts` must assert the compact rail form, one-click enqueue/start handler, and empty-profile guidance remain present.

## Scenario: Review Queue Discovery And Profile Save Feedback

### 1. Scope / Trigger

- Use this when changing commenter routes, sidebar navigation, `ReviewJobsPanel.vue`, `QueueRunsTable.vue`, or `ProjectProfilesPanel.vue`.

### 2. Signatures

```typescript
type CommenterWorkspaceMode = 'project' | 'run' | 'review' | 'global';
type SaveState = 'idle' | 'saving' | 'saved' | 'error';
```

### 3. Contracts

- `/review` is a visible sidebar destination with the global pending-review count.
- The review workspace reuses `QueueRunsTable` to select a run and `ReviewJobsPanel` to act on that run's `review_needed` jobs.
- Review mode requires an explicit accept action before candidate content is written to the source file; the workspace header must state this.
- Project-profile saves disable the submit button while pending and render success or failure feedback adjacent to the action.
- Unsupported settings must be removed rather than displayed as disabled controls that imply future enforcement.

### 4. Validation & Error Matrix

| Case | Expected behavior |
| --- | --- |
| Review jobs exist | Sidebar count is non-zero and `/review` exposes review actions |
| No run is selected | Review panel asks the user to select a run |
| Profile save succeeds | Button re-enables and `commenter.save.success` is visible |
| Profile save fails | Button re-enables and the backend error is shown with `role="alert"` |

### 5. Good/Base/Bad Cases

- Good: a user enters `/review`, selects a run, opens its diff, and explicitly accepts the candidate.
- Base: an empty review queue explains when jobs will appear.
- Bad: `ReviewJobsPanel` exists but is not reachable from navigation, or a save promise rejects without visible feedback.

### 6. Tests Required

- `src/lib/commenterRoute.test.ts` asserts `/review` is mapped to review workspace mode.
- `src/lib/referenceStyleLayout.test.ts` asserts the sidebar entry and review panel remain wired.
- `src/lib/settingsWorkspaceEnhancements.test.ts` asserts project save states and removal of unsupported Token controls.

### 7. Wrong vs Correct

#### Wrong

```vue
<button @click="commenterStore.saveProfile(draft)">保存配置</button>
```

#### Correct

```vue
<button :disabled="saveState === 'saving'" @click="submitProfile">保存配置</button>
<span v-if="saveState === 'error'" role="alert">{{ saveError }}</span>
```

## Scenario: Global Settings Section Navigation

### 1. Scope / Trigger

- Use this when changing `CommentOrchestratorPage.vue`, `DiffToolSettingsPanel.vue`, or the global settings subnavigation.

### 2. Signatures

```typescript
type GlobalSettingsSection =
  | 'api-credentials'
  | 'concurrency-quota'
  | 'diff-tool'
  | 'storage-logs'
  | 'about-settings';
```

### 3. Contracts

- The five left navigation items are interactive selectors, not passive anchors into one long page.
- Exactly one matching settings section is visible in the reference workspace.
- Switching sections preserves the component-level form draft until the user saves or resets it.
- The selected item has a visible active state and exposes its selection state to assistive technology.

### 4. Validation & Error Matrix

| Case | Expected behavior |
| --- | --- |
| Global settings opens | API credentials is selected and is the only visible section |
| User selects Diff tool | Diff tool becomes active and the other four sections are hidden |
| User edits a field and changes sections | Unsaved field value remains when returning |

### 5. Good/Base/Bad Cases

- Good: each left item changes the right content and clearly shows which section is active.
- Base: keyboard focus can reach and activate every navigation button.
- Bad: five links are displayed while all five sections remain visible in one page.

### 6. Tests Required

- `src/lib/settingsWorkspaceEnhancements.test.ts` asserts selected-section state controls `DiffToolSettingsPanel` and passive hash links are absent.
- `pnpm run build` validates the typed section union and Vue template bindings.

### 7. Wrong vs Correct

#### Wrong

```vue
<a href="#diff-tool">Diff 工具</a>
```

#### Correct

```vue
<button :class="{ active: activeSection === 'diff-tool' }" @click="activeSection = 'diff-tool'">
  Diff 工具
</button>
```

## Scenario: Run Header Metrics And File Preview Tabs

### 1. Scope / Trigger

- Use this when changing `RunHeaderStrip.vue` or `StreamContentPanel.vue`.

### 2. Signatures

```typescript
type StreamTab = 'diff' | 'stream' | 'original' | 'request';
```

### 3. Contracts

- The run header omits elapsed-duration and files-per-minute throughput metrics; these values do not help the review workflow and must not consume header space or create a timer.
- Original renders the immutable before-content when available.
- Diff renders original and candidate in independently scrollable split panes; small viewports stack the panes.
- Every content tab owns loading, empty, and error states. The metadata footer must not be the only visible content.

### 4. Validation & Error Matrix

| Case | Expected behavior |
| --- | --- |
| Active or completed run | Header shows progress and outcome counts without elapsed or throughput metrics |
| Original request fails | Inline error is shown |
| Original or candidate is absent | The corresponding pane shows an explicit empty state |

### 5. Good/Base/Bad Cases

- Good: a completed file opens into a populated two-pane Diff view.
- Base: a missing snapshot reports that original content is unavailable.
- Bad: changing the active tab only changes button styling while the body disappears.

### 6. Tests Required

- `src/lib/commenterRunHeader.test.ts` asserts elapsed and throughput metrics remain absent.
- `src/lib/commenterStreamPanel.test.ts` asserts both content branches and the split preview layout.
- `pnpm run build` validates Vue templates and scoped styles.

### 7. Wrong vs Correct

#### Wrong

```typescript
const elapsed_label = computed(() => format_duration(Date.now() - run.started_at));
const throughput_label = computed(() => completed_jobs / elapsed_minutes);
```

#### Correct

```typescript
// Render progress and outcome counts only; do not create a per-second timer.
```
