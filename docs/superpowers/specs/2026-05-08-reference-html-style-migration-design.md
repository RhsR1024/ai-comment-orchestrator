# Reference HTML Style Migration Design

**Date:** 2026-05-08

## Goal

Reshape the current ai-comment-orchestrator frontend into the dark, dense, IDE-like style represented by the two reference HTML files in `docs/`, while preserving current Vue/Tauri behavior and backend contracts.

## Source References

* `docs/B _ _ _ _ RunBar _ _.html` is the run-workspace reference. It shows a compact left rail, top RunBar, current-file strip, file/run/event tabs, file tree, and streaming code reader.
* `docs/_ _ _.html` is the settings reference. It shows the same shell, a settings subnav, compact form fields, and top-right reset/save actions.
* Generated reference screenshots for planning are stored in `.trellis/tasks/05-08-reference-html-style-migration/screenshots/`.

## Current Project State

The app already has the right feature surfaces:

* `src/router/index.ts` routes `/settings` to project configuration and `/workspace` to the run workspace.
* `src/components/Sidebar.vue` already exposes Project Config and Run Workspace as the two primary nav items.
* `src/pages/CommentOrchestratorPage.vue` switches layout by `workspaceMode`.
* Run execution components already exist: `RunHeaderStrip`, `QueueRunsTable`, `RunDetailPanel`, `WorkspaceTreePanel`, `StreamContentPanel`, `ReviewJobsPanel`, `RunHistoryPanel`, and `ExecutionLogPanel`.
* Settings components already exist: `ProjectProfilesPanel` and `DiffToolSettingsPanel`.
* App settings currently include `global_max_workers`, `api_concurrency_limit`, and global `api_bearer_token`.
* Project profile settings currently include `api_base_url`, `api_model`, and `request_timeout_secs`.

This means the migration should mostly be frontend composition and visual system work. It should not introduce a new backend schema.

## Design Decisions

* Keep `/settings` and `/workspace` as separate task-focused surfaces.
* Treat the reference HTML as a visual target, not implementation source.
* Use one shared CSS token system in `src/styles.css` instead of copying huge inline styles from the exported HTML.
* Use existing Vue single-file components and scoped styles; extract small components only when they reduce repeated shell or status UI.
* Use existing `lucide-vue-next` icons for navigation, actions, and status markers.
* Keep card radius restrained. Repeated panels can use 8px radius; global shell blocks can use 0-8px.
* Do not add new runtime data contracts in this migration. Surface existing data more clearly.

## Visual System

### Color Tokens

Use these values as shared CSS variables:

```css
:root {
  --aco-bg: #070b0d;
  --aco-surface-1: #0a0d10;
  --aco-surface-2: #0f1316;
  --aco-surface-3: #141a1f;
  --aco-border: rgba(108, 142, 164, 0.2);
  --aco-border-strong: rgba(108, 142, 164, 0.34);
  --aco-text: #e6edf3;
  --aco-muted: #9ba9b4;
  --aco-subtle: #6c7a85;
  --aco-teal: #5cd3c8;
  --aco-green: #34d399;
  --aco-blue: #7aa2f7;
  --aco-yellow: #f5b942;
  --aco-red: #ef5a6f;
}
```

### Typography

* Body text: Inter-style system stack: `Inter, -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif`.
* Code, paths, run keys, counters: `JetBrains Mono, ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace`.
* Default body size should be 12-13px in dense panels, with 14px reserved for section headings and important labels.
* Letter spacing should remain `0` except uppercase micro-labels where `0.04em` is acceptable.

### Surfaces

* Shell background is flat near-black, not a decorative gradient.
* Panels use thin borders and low-contrast backgrounds.
* Avoid nested cards. The reference is divided by rails, panels, and separators, not by stacked marketing cards.
* Important state uses small pills/chips and icon accents.

### Interaction

* Hover/focus states must not scale or shift layout.
* Use 150-220ms color/border/background transitions.
* The stream cursor may blink; it must be disabled under `prefers-reduced-motion: reduce`.

## App Shell

### Sidebar

`Sidebar.vue` should become a fixed dark rail matching the reference:

* Width: `232px` desktop.
* Brand: square `AC` mark, `ACO`, and `Comment Orchestrator`.
* Main nav:
  * Project Config with a small count badge from `commenterStore.state.profiles.length`.
  * Run Workspace with a small count badge from active/review work.
* Bottom status block:
  * API online/offline style derived from whether a global token is present.
  * Worker/API capacity text from `app_settings`.

The sidebar should call `commenterStore.initialize()` indirectly through the page or accept already-loaded state. It should not create a second polling flow.

### App Header

The current top app header should be minimized. The reference puts page-specific controls inside each page. Keep the locale switch available but make it secondary, either in the sidebar footer or a compact top-right utility region.

## Settings Workspace

`/settings` should visually match `docs/_ _ _.html`.

### Layout

Desktop grid:

```text
sidebar 232px | settings subnav 240px | form content minmax(0, 1fr)
```

Main content:

* Header row: icon, `Global Settings`, short description, Reset Default, Save.
* Left settings subnav:
  * API Credential
  * Concurrency Quota
  * Diff Tool
  * Storage & Logs
  * About
  * Project Profiles
* Content column shows sections as flat form groups separated by spacing.

### Sections

API Credential:

* Bind global `api_bearer_token` from `app_settings`.
* Display masked token input and a Replace action.
* Display API base URL/model/timeout in the project profile section, because those fields currently live on `CommenterProjectSettings`.
* Do not move `api_base_url` into app settings without a separate backend/data-contract task.

Concurrency Quota:

* Bind `global_max_workers`.
* Bind `api_concurrency_limit`.
* Display single-file max token as profile-scoped or disabled explanatory value only if no current field exists. The implementation should not invent persisted storage for it.

Diff Tool:

* Bind `command_template` from `diff_tool_settings`.
* Keep `{before}` and `{after}` helper copy.

Project Profiles:

* Keep root path, include extensions, exclude directories, default mode, max files, prompt template, and allow-light-rewrite.
* Add visible fields for existing `api_base_url`, `api_model`, and `request_timeout_secs` so the existing runtime endpoint controls are no longer hidden.
* Keep profile list compact and scannable.

### Save Model

Settings page should maintain local draft state for global settings and diff settings, then save both from the top-level Save action. Project profile Save remains inside the profile section because it creates/updates a project-specific record.

## Run Workspace

`/workspace` should visually match `docs/B _ _ _ _ RunBar _ _.html`.

### Layout

Desktop grid:

```text
sidebar 232px | main workspace

main workspace:
  RunBar
  current-file strip
  body grid:
    left rail 360px
    right stream area minmax(0, 1fr)
```

### RunBar

`RunHeaderStrip.vue` should become a compact telemetry bar:

* Identity block:
  * Project/profile name.
  * Run status.
  * Run key.
  * Run mode.
  * Model if available from the selected profile.
* Progress block:
  * Completed/total.
  * Progress percent.
  * Small segmented progress meter.
* Runtime metrics:
  * Elapsed time from `started_at` to now or `finished_at`.
  * Throughput as completed files per minute when elapsed time is positive.
  * Token counters are shown only if data exists; otherwise the UI reserves no fake field.
* Issue chips:
  * Review needed.
  * Failed.
  * Completed.
  * Skipped.
* Actions:
  * Pause/Resume.
  * Cancel.
  * More actions can stay icon-only with accessible labels.

### Current-File Strip

Below RunBar, show:

* Current file path or idle text.
* Character/chunk metrics when available from live stream slices.
* TTFT only if a real event timestamp can be derived; otherwise omit.
* Right arrow or focus action if it selects the current file.

### Body Left Rail

The left rail should combine:

* Tabs for Files, Runs, and Events.
* File tab renders `WorkspaceTreePanel`.
* Runs tab renders a compact queue list based on `QueueRunsTable` data but not the full enqueue form.
* Events tab renders `ExecutionLogPanel`.
* The enqueue form can be a compact top action or collapsed panel so the run-reading surface stays primary.

### Body Right Rail

The right rail should focus on the selected file:

* Header with file path, language hint, size/line data when available, status, mode, and encoding display.
* Subtabs: Diff, Streaming Response, Original, File Events.
* Primary content is `StreamContentPanel`.
* Review actions for the selected file should be near the stream header when status is `review_needed`.

## Data Mapping

| Reference Concept | Current Source |
| --- | --- |
| Project/profile name | `selected_run_detail.run.profile_key`, `profiles` lookup |
| Run key | `selected_run_detail.run.run_key` |
| Run mode | `selected_run_detail.run.run_mode` |
| Model | selected profile `settings.api_model` |
| Progress | `completed_jobs`, `total_jobs`, `run_progress_percent()` |
| Review chip | `review_needed_jobs` |
| Failed chip | `failed_jobs` |
| Skipped chip | `skipped_jobs` |
| Worker capacity | `run.max_workers`, `app_settings.global_max_workers` |
| API capacity | `app_settings.api_concurrency_limit` |
| Current file | `run.current_file` or selected tree file |
| Stream content | `live_streams` plus `getCandidateText()` fallback |
| File tree | `listDir()` plus selected run jobs |
| Events | `selected_run_detail.events` and `execution_logs` |

## Error And Empty States

* No selected run: render an idle RunBar and empty body message, not a blank panel.
* Missing token: settings API status block should show an actionable warning; run execution behavior stays backend-driven.
* Root path inaccessible: keep `WorkspaceTreePanel` retry state.
* Candidate unavailable: keep `StreamContentPanel` error/empty state.
* Small screens: stack subnav above content and body rails vertically.

## Testing Strategy

Add or update source-level smoke tests:

* `commenterRoute.test.ts` keeps `/settings` and `/workspace` route assertions.
* `settingsWorkspaceEnhancements.test.ts` asserts settings subnav, top save/reset actions, project profile endpoint fields, and compact section class names.
* `commenterRunHeader.test.ts` asserts RunBar telemetry helpers/classes and accessible action buttons.
* `commenterStreamPanel.test.ts` asserts live/locked mode, stream tabs, and `prefers-reduced-motion` cursor handling.
* New `referenceStyleLayout.test.ts` asserts shared CSS tokens and absence of obsolete spacious summary-card-first layout.

Manual visual verification:

* Run Vite dev server.
* Capture `/settings` and `/workspace` at 1440x920 and 375x812 with Playwright.
* Compare against the reference screenshots for shell density, color, and hierarchy.

## Out Of Scope

* Importing exported reference HTML/CSS into the app.
* Adding a UI component library.
* Moving API base URL/model from project settings to app settings.
* Changing Tauri command names or backend database schema.
* Implementing real token usage metrics unless backend events already expose them.
