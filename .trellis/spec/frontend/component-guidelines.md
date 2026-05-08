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
