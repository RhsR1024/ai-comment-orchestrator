# Reference HTML Style Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rework the current Vue/Tauri ai-comment-orchestrator UI to match the two dark IDE-style reference HTML designs in `docs/`.

**Architecture:** Keep existing Tauri commands, store, and DTOs. Introduce a shared CSS token system, restructure the information architecture into **three top-level workspaces** (Project Config / Run Workspace / Global Settings), and reshape the sidebar, settings shells, and run shell using existing commenter components.

**Tech Stack:** Vue 3 SFCs, TypeScript, Vue Router, Vite, `lucide-vue-next`, existing `commenterStore`, smoke tests via `tsx`, Playwright for screenshot verification.

---

## Audit-driven Adjustments (2026-05-08)

This revision incorporates findings from the design audit against the two reference screenshots in `docs/`. The changes below override the earlier draft of this plan.

### Drop English locale, lock to Chinese single-language baseline

The product is Chinese-only going forward. Plan removes the `en-US` map, locale switch UI, and locale storage so subsequent work does not have to maintain parallel translations. The reference screenshots are entirely Chinese — keeping a runtime locale switch creates work without a real consumer.

### Information architecture: three top-level workspaces, not two

The reference settings screenshot shows three sidebar entries: `项目配置 (4)`, `运行工作区 (2)`, and `全局设置`. The previous draft conflated `项目配置` with `全局设置` and stuffed `Project Profiles` into the global subnav. The corrected layout:

| Route | Sidebar label | Content |
| --- | --- | --- |
| `/settings` | 项目配置 | `ProjectProfilesPanel` only |
| `/workspace` | 运行工作区 | RunBar + run shell |
| `/global` (new) | 全局设置 | API 凭证 / 并发配额 / Diff 工具 / 存储与日志 / 关于 |

`CommentOrchestratorPage.vue` extends its `WorkspaceMode` to `'project' | 'run' | 'global'`.

### Visual-degradation policy for unbacked elements

Several reference elements have no backend source today and the spec keeps backend out of scope. Each element gets an explicit policy in this plan instead of being silently dropped:

| Reference element | Backend source | Policy this migration |
| --- | --- | --- |
| RunBar throughput sparkline | derive from `started_at`/`completed_jobs` | text-only `throughput_label`, no chart |
| RunBar token gauge (`259K 入 218 / 出 41`) | none | hide the entire `runbar-tokens` block when no data exists |
| RunBar TTFT (`520ms`) | none | hide chip when no real value is derivable |
| Credential `凭证已验证 · 上次握手` pill | none | render disabled gray placeholder reading `凭证未校验`; no fake success state |
| Single-file max token (`8000`) | none | render disabled input with hint `后端暂未支持，下个版本接入`; cannot be saved |
| Storage & Logs section | none | render section with two read-only rows (`数据库目录 / 日志目录`) showing values from `app_settings` if present, otherwise `—` |
| About section | static | render version/build hash/license placeholder copy |

### Gradient and decorative-fill cleanup

The previous draft only asserted `radial-gradient` was removed. The reference is flat. This revision explicitly removes:
* `linear-gradient(135deg, ...)` from `RunHeaderStrip` background.
* `linear-gradient(180deg, ...)` from the sidebar mark.
* `linear-gradient(90deg, #34d399, #22c55e)` from the progress track (replaced with solid `--aco-green`).
* Any `box-shadow` on `.panel`, `.settings-summary-card`.

### Run-workspace left-rail tab wiring

The previous draft only rendered Files. Now the three tabs (`文件 / Runs / 事件`) each render real content, gated by an `active_left_tab` ref, with badge counts pulled from store state.

### Stream-meta richer set

`stream-meta` block now includes language tag (file-extension lookup), size (KB), line count, chunk count — matching the reference. Prior draft only had `UTF-8 / LF / X chars`.

### Capacity label format

Sidebar capacity label changes from `{workers} workers / {api} API` to `{used} / {api} 并发` with `{used}` derived from `commenterStore.state.runs.filter(r => !r.finished_at).length`. A green dot status indicator sits next to `API · 在线` to match the reference.

### Out of scope explicitly reaffirmed

* No moving `api_base_url` / `api_model` / `request_timeout_secs` from project profile to app settings.
* No new persisted fields (single-file max token, credential handshake timestamp, token-usage counters).
* No locale runtime — Chinese-only.

---

## File Structure

* Modify: `src/styles.css` for shared tokens, shell grid, dense controls, responsive rules, and gradient cleanup.
* Modify: `src/App.vue` to drop the locale switch and tighten the shell.
* Modify: `src/components/Sidebar.vue` for the three-item nav and bottom status card.
* Modify: `src/router/index.ts` to add `/global`.
* Modify: `src/pages/CommentOrchestratorPage.vue` to switch between `project` / `run` / `global` modes.
* Modify: `src/components/commenter/DiffToolSettingsPanel.vue` for the global settings sections, save/reset expose, and disabled placeholders for unbacked fields.
* Modify: `src/components/commenter/ProjectProfilesPanel.vue` for compact reference look (no global-settings fields here).
* Modify: `src/components/commenter/RunHeaderStrip.vue` for RunBar layout, gradient removal, and unbacked-block degradation.
* Modify: `src/components/commenter/RunDetailPanel.vue` for the three-tab left rail.
* Modify: `src/components/commenter/WorkspaceTreePanel.vue` for compact appearance.
* Modify: `src/components/commenter/StreamContentPanel.vue` for stream tabs and full meta.
* Modify: `src/components/commenter/ExecutionLogPanel.vue` for events rail density.
* Modify: `src/locales/messages.ts` to remove `en-US`, `LocaleCode` union, locale storage, and `locale_options`.
* Modify tests:
  * `src/lib/settingsWorkspaceEnhancements.test.ts`
  * `src/lib/commenterRunHeader.test.ts`
  * `src/lib/commenterStreamPanel.test.ts`
  * `src/lib/commenterLocale.test.ts`
  * `src/lib/commenterRoute.test.ts`
* Create: `src/lib/referenceStyleLayout.test.ts`.

---

### Task 0: Drop English Locale And Lock Chinese Single-language Baseline

**Files:**
* Modify: `src/locales/messages.ts`
* Modify: `src/App.vue`
* Modify: `src/lib/commenterLocale.test.ts`

- [ ] **Step 1: Strip English map and locale plumbing**

In `src/locales/messages.ts`:

* Remove the `'en-US'` block from `MESSAGES`.
* Replace `export type LocaleCode = 'zh-CN' | 'en-US'` with `export type LocaleCode = 'zh-CN'`.
* Remove `LOCALE_STORAGE_KEY`, `parse_stored_locale`, `load_initial_locale`, `set_locale`, the `active_locale` `ref`, and any `localStorage` access.
* Replace the `MESSAGES` constant with a single `Record<string, string>` named `MESSAGES_ZH`.
* Simplify `resolve_message` to `(key, params?) => apply_params(MESSAGES_ZH[key] ?? key, params)`.
* Remove `locale`, `set_locale`, and `locale_options` from `use_messages`. Keep `t` only.

- [ ] **Step 2: Remove the locale switch from `App.vue`**

In `src/App.vue`:

* Drop `locale`, `locale_options`, `set_locale` from the destructured `use_messages()` return.
* Remove the entire `<header class="app-header">` block (brand + locale switch). Move the brand to the sidebar (Task 3).
* Remove the `<style scoped>` block for `.app-header-brand`, `.app-title`, `.locale-switch`, `.locale-segment`, `.locale-option`.

- [ ] **Step 3: Update the locale test**

Replace `src/lib/commenterLocale.test.ts` body with assertions that lock the single-language contract:

```ts
import assert from 'node:assert/strict';
import fs from 'node:fs';

const source = fs.readFileSync(new URL('../locales/messages.ts', import.meta.url), 'utf8');
const app_source = fs.readFileSync(new URL('../App.vue', import.meta.url), 'utf8');

assert.doesNotMatch(source, /'en-US'/, "en-US locale must be removed");
assert.doesNotMatch(source, /set_locale/, "locale switching must be removed");
assert.doesNotMatch(source, /localStorage/, "locale storage must be removed");
assert.match(source, /export type LocaleCode = 'zh-CN'/, 'LocaleCode should be Chinese-only');
assert.doesNotMatch(app_source, /locale_options/, 'App.vue should not render a locale switch');
assert.doesNotMatch(app_source, /class="app-header"/, 'App.vue should drop the legacy app header');

console.log('commenter locale PASSED');
```

- [ ] **Step 4: Run the locale test**

```bash
pnpm exec tsx src/lib/commenterLocale.test.ts
```

Expected: passes after Steps 1-3 are done.

### Task 1: Lock Reference Style Contracts With Tests

**Files:**
* Modify: `src/lib/settingsWorkspaceEnhancements.test.ts`
* Modify: `src/lib/commenterRunHeader.test.ts`
* Modify: `src/lib/commenterStreamPanel.test.ts`
* Modify: `src/lib/commenterRoute.test.ts`
* Create: `src/lib/referenceStyleLayout.test.ts`
* Modify: `package.json`

- [ ] **Step 1: Extend the route test for the new `/global` route**

Add to `src/lib/commenterRoute.test.ts` assertions covering all three top-level routes:

```ts
import assert from 'node:assert/strict';
import fs from 'node:fs';

const router_source = fs.readFileSync(new URL('../router/index.ts', import.meta.url), 'utf8');

for (const path of ["'/settings'", "'/workspace'", "'/global'"]) {
  assert.match(router_source, new RegExp(path), `${path} route should exist`);
}

assert.match(router_source, /workspaceMode: 'project'/, 'project mode should be wired to /settings');
assert.match(router_source, /workspaceMode: 'run'/, 'run mode should be wired to /workspace');
assert.match(router_source, /workspaceMode: 'global'/, 'global mode should be wired to /global');

console.log('commenter route PASSED');
```

- [ ] **Step 2: Extend the settings workspace test**

Replace the assertions in `src/lib/settingsWorkspaceEnhancements.test.ts` with:

```ts
import assert from 'node:assert/strict';
import fs from 'node:fs';

const settings_page = new URL('../pages/CommentOrchestratorPage.vue', import.meta.url);
const profiles_panel = new URL('../components/commenter/ProjectProfilesPanel.vue', import.meta.url);
const diff_panel = new URL('../components/commenter/DiffToolSettingsPanel.vue', import.meta.url);
const messages_file = new URL('../locales/messages.ts', import.meta.url);

const settings_page_source = fs.readFileSync(settings_page, 'utf8');
assert.match(settings_page_source, /global-reference-shell/, 'global settings shell should exist');
assert.match(settings_page_source, /project-reference-shell/, 'project config shell should exist');
assert.match(settings_page_source, /global-subnav/, 'global settings should expose a subnav');
assert.match(settings_page_source, /global-top-actions/, 'global settings should expose reset/save actions');
assert.match(settings_page_source, /workspaceMode === 'project'/, 'project mode branch must remain');
assert.match(settings_page_source, /workspaceMode === 'global'/, 'global mode branch must exist');

const profiles_panel_source = fs.readFileSync(profiles_panel, 'utf8');
assert.match(profiles_panel_source, /profile-form-grid/, 'project profile fields should keep the established form grid');
for (const field of ['api_base_url', 'api_model', 'request_timeout_secs']) {
  assert.match(profiles_panel_source, new RegExp(field), `${field} should remain in project profile settings`);
}

const diff_panel_source = fs.readFileSync(diff_panel, 'utf8');
assert.match(diff_panel_source, /defineExpose/, 'global settings panel should expose save/reset methods');
assert.match(diff_panel_source, /api_bearer_token/, 'global API token should stay in app settings');
assert.match(diff_panel_source, /credentials-status-pill/, 'global settings should render a verified-credential placeholder pill');
assert.match(diff_panel_source, /single-file-token-placeholder/, 'global settings should render the disabled single-file token placeholder');

const messages_source = fs.readFileSync(messages_file, 'utf8');
for (const key of [
  'global.title',
  'global.help',
  'global.section.apiCredentials',
  'global.section.concurrencyQuota',
  'global.section.diffTool',
  'global.section.storageLogs',
  'global.section.about',
  'global.resetDefaults',
  'global.credential.notVerified',
  'global.singleFileToken.disabled',
  'global.storage.databaseDir',
  'global.storage.logDir',
  'global.about.version'
]) {
  assert.equal(messages_source.includes(`'${key}'`), true, `${key} should exist in locale messages`);
}

console.log('settings workspace enhancements PASSED');
```

- [ ] **Step 3: Extend the run header test**

Update `src/lib/commenterRunHeader.test.ts` to assert RunBar contracts including degradation rules:

```ts
import assert from 'node:assert/strict';
import fs from 'node:fs';

const source = fs.readFileSync(
  new URL('../components/commenter/RunHeaderStrip.vue', import.meta.url),
  'utf8'
);

for (const token of [
  'runbar',
  'runbar-identity',
  'runbar-progress',
  'runbar-metrics',
  'runbar-issues',
  'runbar-actions'
]) {
  assert.match(source, new RegExp(token), `${token} should be part of the reference RunBar`);
}

assert.match(source, /elapsed_label/, 'RunBar should derive elapsed time text');
assert.match(source, /throughput_label/, 'RunBar should derive throughput text');
assert.match(source, /aria-label/, 'RunBar icon actions should keep accessible labels');
assert.doesNotMatch(source, /linear-gradient\(135deg/, 'RunBar should not retain decorative gradient backgrounds');
assert.doesNotMatch(source, /linear-gradient\(90deg, #34d399/, 'progress track should not retain the multi-stop gradient');
assert.match(source, /v-if="show_token_block"/, 'RunBar token block must hide when no data is available');
assert.match(source, /v-if="show_ttft_chip"/, 'TTFT chip must hide when no real value is derivable');

console.log('commenter run header PASSED');
```

- [ ] **Step 4: Extend the stream panel test**

Update `src/lib/commenterStreamPanel.test.ts`:

```ts
import assert from 'node:assert/strict';
import fs from 'node:fs';

const source = fs.readFileSync(
  new URL('../components/commenter/StreamContentPanel.vue', import.meta.url),
  'utf8'
);
const styles = fs.readFileSync(new URL('../styles.css', import.meta.url), 'utf8');

assert.match(source, /'live'/, "stream panel should reference 'live' mode");
assert.match(source, /'locked'/, "stream panel should reference 'locked' mode");
assert.match(source, /commenterApi\.getCandidateText/, 'stream panel should fetch candidate text on demand');
assert.match(source, /stream-tabs/, 'stream panel should expose reference-style stream tabs');
assert.match(source, /stream-meta/, 'stream panel should expose file and stream metadata');
assert.match(source, /size_kb_label/, 'stream panel should derive file size in KB');
assert.match(source, /line_count_label/, 'stream panel should derive line count');
assert.match(source, /chunk_count_label/, 'stream panel should derive chunk count');
assert.match(source, /language_label/, 'stream panel should derive language tag from path');
assert.match(styles, /prefers-reduced-motion/, 'shared styles should respect reduced motion');

console.log('commenter stream panel PASSED');
```

- [ ] **Step 5: Create a shared layout token test**

Create `src/lib/referenceStyleLayout.test.ts`:

```ts
import assert from 'node:assert/strict';
import fs from 'node:fs';

const styles = fs.readFileSync(new URL('../styles.css', import.meta.url), 'utf8');
const app = fs.readFileSync(new URL('../App.vue', import.meta.url), 'utf8');
const sidebar = fs.readFileSync(new URL('../components/Sidebar.vue', import.meta.url), 'utf8');
const page = fs.readFileSync(new URL('../pages/CommentOrchestratorPage.vue', import.meta.url), 'utf8');
const run_header = fs.readFileSync(
  new URL('../components/commenter/RunHeaderStrip.vue', import.meta.url),
  'utf8'
);
const run_detail = fs.readFileSync(
  new URL('../components/commenter/RunDetailPanel.vue', import.meta.url),
  'utf8'
);

for (const token of [
  '--aco-bg',
  '--aco-surface-1',
  '--aco-surface-2',
  '--aco-border',
  '--aco-teal',
  '--aco-green',
  '--aco-blue',
  '--aco-yellow',
  '--aco-red'
]) {
  assert.match(styles, new RegExp(token), `${token} should be defined as a shared style token`);
}

assert.match(app, /app-shell--reference/, 'app shell should use the reference shell class');
assert.match(sidebar, /sidebar-status-card/, 'sidebar should expose bottom API status');
assert.match(sidebar, /sidebar-status-dot/, 'sidebar should render a colored API status dot');
assert.match(sidebar, /go\('\/global'\)/, 'sidebar should link to the new /global route');
assert.match(page, /run-reference-shell/, 'run workspace should use the reference run shell');
assert.doesNotMatch(styles, /radial-gradient/, 'reference shell should avoid decorative gradient backgrounds');
assert.doesNotMatch(sidebar, /linear-gradient\(180deg, rgba\(95, 212, 204/, 'sidebar mark should be flat');
assert.match(run_detail, /active_left_tab/, 'run detail must drive its left rail with a tab ref');
assert.match(run_detail, /'files'.*'runs'.*'events'/s, 'run detail must expose all three tabs');
assert.match(run_detail, /v-if="active_left_tab === 'runs'"/, 'runs tab must render its content');
assert.match(run_detail, /v-if="active_left_tab === 'events'"/, 'events tab must render its content');
assert.doesNotMatch(run_header, /linear-gradient\(135deg/, 'run header must not keep decorative gradient');

console.log('reference style layout PASSED');
```

- [ ] **Step 6: Add the new test to the smoke command**

In `package.json`, append the new test near the other UI source tests:

```json
"smoke": "tsx src/lib/commenterApiShape.test.ts && tsx src/lib/commenterRoute.test.ts && tsx src/lib/commenterLocale.test.ts && tsx src/lib/commenterProfileDefaults.test.ts && tsx src/lib/commenterCredentialPanel.test.ts && tsx src/lib/settingsWorkspaceCleanup.test.ts && tsx src/lib/settingsWorkspaceEnhancements.test.ts && tsx src/lib/referenceStyleLayout.test.ts && tsx src/lib/commenterExecutionLog.test.ts && tsx src/lib/commenterFileLog.test.ts && tsx src/lib/commenterStreamSlice.test.ts && tsx src/lib/commenterRunHeader.test.ts && tsx src/lib/commenterWorkspaceTree.test.ts && tsx src/lib/commenterStreamPanel.test.ts"
```

- [ ] **Step 7: Run the new failing tests**

```bash
pnpm exec tsx src/lib/commenterRoute.test.ts
pnpm exec tsx src/lib/settingsWorkspaceEnhancements.test.ts
pnpm exec tsx src/lib/commenterRunHeader.test.ts
pnpm exec tsx src/lib/commenterStreamPanel.test.ts
pnpm exec tsx src/lib/referenceStyleLayout.test.ts
```

Expected before implementation: each fails on at least one assertion because the new classes, route, and tokens do not exist yet.

### Task 2: Add Shared Reference Style Tokens

**Files:**
* Modify: `src/styles.css`
* Modify: `src/App.vue`

- [ ] **Step 1: Replace the global decorative background with reference tokens**

At the top of `src/styles.css`, replace the current `:root` block with:

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
  color: var(--aco-text);
  background: var(--aco-bg);
  font-family:
    Inter,
    -apple-system,
    BlinkMacSystemFont,
    "Segoe UI",
    system-ui,
    sans-serif;
  line-height: 1.5;
  font-weight: 400;
}
```

- [ ] **Step 2: Add shared mono utility**

```css
.mono {
  font-family:
    "JetBrains Mono",
    ui-monospace,
    SFMono-Regular,
    Menlo,
    Monaco,
    Consolas,
    "Liberation Mono",
    monospace;
}
```

- [ ] **Step 3: Tighten app shell layout and remove `.app-header` rules**

```css
.app-shell,
.app-shell--reference {
  display: grid;
  grid-template-columns: 232px minmax(0, 1fr);
  min-height: 100vh;
  background: var(--aco-bg);
}

.app-main {
  display: flex;
  min-width: 0;
  flex-direction: column;
  padding: 0;
  gap: 0;
  border-left: 1px solid rgba(108, 142, 164, 0.12);
  background: var(--aco-bg);
}
```

Delete the old `.app-header`, `.page-shell`, `.page-intro` blocks; the new shells in subsequent tasks own their headers.

- [ ] **Step 4: Strip box shadows and decorative fills from existing shared classes**

* On `.panel`: remove `box-shadow`, swap background to `var(--aco-surface-1)`, swap border to `1px solid var(--aco-border)`.
* On `.settings-summary-card`, `.metric-card`, `.list-item`: remove `box-shadow`, replace gradient/translucent backgrounds with flat `var(--aco-surface-2)`.
* On `.progress > span`: replace gradient with `background: var(--aco-green);`.
* On `.button`: replace `rgba(20, 72, 77, 0.4)` background with `var(--aco-surface-2)` and use `border: 1px solid var(--aco-border)`.

- [ ] **Step 5: Add reduced-motion support**

```css
@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    scroll-behavior: auto !important;
    transition-duration: 0.01ms !important;
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
  }
}
```

- [ ] **Step 6: Mark the app shell**

In `src/App.vue`, the post-Task-0 template should be:

```vue
<template>
  <div class="app-shell app-shell--reference">
    <Sidebar />
    <main class="app-main">
      <RouterView />
    </main>
  </div>
</template>
```

- [ ] **Step 7: Run token test**

```bash
pnpm exec tsx src/lib/referenceStyleLayout.test.ts
```

Expected: token assertions and `.app-shell--reference` assertion pass; later assertions still fail until the relevant components are reshaped.

### Task 3: Rebuild The Sidebar Rail With Three Nav Items

**Files:**
* Modify: `src/router/index.ts`
* Modify: `src/components/Sidebar.vue`
* Modify: `src/locales/messages.ts`
* Modify: `src/styles.css`

- [ ] **Step 1: Add the `/global` route**

In `src/router/index.ts`, extend `CommenterWorkspaceMode` and add the route:

```ts
type CommenterWorkspaceMode = 'project' | 'run' | 'global';

// inside routes array
{
  path: '/global',
  component: commenter_page,
  props: {
    workspaceMode: 'global' satisfies CommenterWorkspaceMode
  }
}
```

Keep the existing `/settings` and `/workspace` entries; add a `/` redirect to `/workspace` if you also want the run workspace to be the default landing surface (optional — keep the existing `/settings` redirect if unchanged).

- [ ] **Step 2: Wire store-backed sidebar counts and state**

In `Sidebar.vue`:

```ts
import { computed } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { Box, Globe2, SquareActivity } from 'lucide-vue-next';

import { commenterStore } from '../lib/commenterStore';
import { use_messages } from '../locales/messages';

const route = useRoute();
const router = useRouter();
const { t } = use_messages();

const profile_count = computed(() => commenterStore.state.profiles.length);
const run_attention_count = computed(
  () =>
    commenterStore.state.review_jobs.length +
    commenterStore.state.runs.filter((run) => !run.finished_at).length
);

const has_token = computed(() =>
  Boolean(commenterStore.state.app_settings?.api_bearer_token.trim())
);
const api_status_label = computed(() =>
  has_token.value ? t('sidebar.apiOnline') : t('sidebar.apiMissing')
);
const concurrency_used = computed(
  () => commenterStore.state.runs.filter((run) => !run.finished_at).length
);
const concurrency_max = computed(
  () => commenterStore.state.app_settings?.api_concurrency_limit ?? 0
);
const capacity_label = computed(() =>
  t('sidebar.capacity', { used: concurrency_used.value, max: concurrency_max.value })
);

function is_active(path: string) {
  return route.path === path;
}

function go(path: string) {
  void router.push(path);
}
```

- [ ] **Step 3: Replace sidebar template (three-item nav)**

```vue
<aside class="sidebar">
  <div class="sidebar-brand">
    <div class="sidebar-mark">AC</div>
    <div>
      <strong>ACO</strong>
      <p>Comment Orchestrator</p>
    </div>
  </div>

  <nav class="sidebar-nav" aria-label="Workspaces">
    <button class="sidebar-link" :class="{ active: is_active('/settings') }" @click="go('/settings')">
      <Box :size="15" />
      <span>{{ t('nav.projectConfig') }}</span>
      <span class="sidebar-count">{{ profile_count }}</span>
    </button>
    <button class="sidebar-link" :class="{ active: is_active('/workspace') }" @click="go('/workspace')">
      <SquareActivity :size="15" />
      <span>{{ t('nav.runWorkspace') }}</span>
      <span class="sidebar-count sidebar-count--active">{{ run_attention_count }}</span>
    </button>
    <button class="sidebar-link" :class="{ active: is_active('/global') }" @click="go('/global')">
      <Globe2 :size="15" />
      <span>{{ t('nav.globalSettings') }}</span>
    </button>
  </nav>

  <div class="sidebar-spacer" />

  <div class="sidebar-status-card">
    <div class="sidebar-status-line">
      <span class="sidebar-status-dot" :class="has_token ? 'sidebar-status-dot--online' : 'sidebar-status-dot--offline'" />
      <strong>{{ api_status_label }}</strong>
    </div>
    <p>{{ capacity_label }}</p>
  </div>
</aside>
```

- [ ] **Step 4: Add or update locale keys (Chinese-only)**

In `src/locales/messages.ts` (now a single Chinese map):

```ts
'nav.projectConfig': '项目配置',
'nav.runWorkspace': '运行工作区',
'nav.globalSettings': '全局设置',
'sidebar.apiOnline': 'API · 在线',
'sidebar.apiMissing': '缺少 API Token',
'sidebar.capacity': '{used} / {max} 并发',
```

- [ ] **Step 5: Restyle the sidebar (flat, no gradients)**

Add to scoped or global styles:

```css
.sidebar {
  display: flex;
  flex-direction: column;
  gap: 18px;
  min-height: 100vh;
  background: var(--aco-surface-1);
  border-right: 1px solid var(--aco-border);
  padding: 18px 14px;
}

.sidebar-mark {
  display: grid;
  place-items: center;
  width: 32px;
  height: 32px;
  border-radius: 8px;
  background: var(--aco-surface-3);
  color: var(--aco-teal);
  font-size: 12px;
  font-weight: 700;
}

.sidebar-link {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--aco-muted);
  cursor: pointer;
  padding: 8px 10px;
  text-align: left;
  font-size: 13px;
}

.sidebar-link.active {
  background: var(--aco-surface-2);
  color: var(--aco-text);
}

.sidebar-count {
  margin-left: auto;
  border-radius: 999px;
  background: var(--aco-surface-3);
  color: var(--aco-muted);
  padding: 1px 7px;
  font-size: 11px;
}

.sidebar-count--active {
  color: var(--aco-yellow);
}

.sidebar-spacer {
  flex: 1;
}

.sidebar-status-card {
  display: flex;
  flex-direction: column;
  gap: 4px;
  border: 1px solid var(--aco-border);
  border-radius: 8px;
  background: var(--aco-surface-2);
  padding: 8px 10px;
  font-size: 12px;
  color: var(--aco-muted);
}

.sidebar-status-line {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--aco-text);
}

.sidebar-status-dot {
  width: 6px;
  height: 6px;
  border-radius: 999px;
}

.sidebar-status-dot--online {
  background: var(--aco-green);
}

.sidebar-status-dot--offline {
  background: var(--aco-red);
}
```

The sidebar mark must not retain `linear-gradient(180deg, rgba(95, 212, 204, 0.3)...)`.

- [ ] **Step 6: Run sidebar-related tests**

```bash
pnpm exec tsx src/lib/commenterRoute.test.ts
pnpm exec tsx src/lib/referenceStyleLayout.test.ts
```

Expected: route test passes once `/global` exists; layout test passes its sidebar status, dot, and `/global` link assertions.

### Task 4: Recompose The Project Config Workspace

**Files:**
* Modify: `src/pages/CommentOrchestratorPage.vue`
* Modify: `src/components/commenter/ProjectProfilesPanel.vue`
* Modify: `src/locales/messages.ts`

- [ ] **Step 1: Branch the page by `workspaceMode`**

The page now has three branches. Add an early skeleton:

```vue
<template>
  <section v-if="props.workspaceMode === 'project'" class="project-reference-shell">
    <header class="project-reference-header">
      <div class="project-title-row">
        <Box class="project-title-icon" :size="15" aria-hidden="true" />
        <div>
          <h1>{{ t('project.title') }}</h1>
          <p>{{ t('project.help') }}</p>
        </div>
      </div>
    </header>
    <div class="project-reference-content">
      <ProjectProfilesPanel variant="reference" />
    </div>
  </section>

  <section v-else-if="props.workspaceMode === 'global'" class="global-reference-shell">
    <!-- Task 5 -->
  </section>

  <section v-else class="run-reference-shell">
    <!-- Task 7 -->
  </section>
</template>
```

Add to the script setup:

```ts
import { Box, Globe2 } from 'lucide-vue-next';

type WorkspaceMode = 'project' | 'run' | 'global';
const props = withDefaults(defineProps<{ workspaceMode?: WorkspaceMode }>(), {
  workspaceMode: 'project'
});
```

- [ ] **Step 2: Add `variant` prop to `ProjectProfilesPanel`**

```ts
const props = withDefaults(defineProps<{ variant?: 'panel' | 'reference' }>(), {
  variant: 'panel'
});
```

Apply `:class="['panel', { 'project-form-panel': props.variant === 'reference' }]"` to the outer section. The panel keeps its own `Save Profile` button — the project page does not provide a top-level save (saving a profile is per-record CRUD).

- [ ] **Step 3: Add locale keys**

```ts
'project.title': '项目配置',
'project.help': '维护多个项目的根路径、过滤规则和 Prompt 模板。',
```

- [ ] **Step 4: Run settings tests**

```bash
pnpm exec tsx src/lib/settingsWorkspaceEnhancements.test.ts
pnpm exec tsx src/lib/commenterProfileDefaults.test.ts
```

Expected: profile defaults test still passes; the settings-workspace test fails on `global-reference-shell` until Task 5 lands.

### Task 5: Build The Global Settings Workspace

**Files:**
* Modify: `src/pages/CommentOrchestratorPage.vue`
* Modify: `src/components/commenter/DiffToolSettingsPanel.vue`
* Modify: `src/locales/messages.ts`

- [ ] **Step 1: Render the global shell template**

In `CommentOrchestratorPage.vue`, replace the `v-else-if="props.workspaceMode === 'global'"` branch with:

```vue
<section v-else-if="props.workspaceMode === 'global'" class="global-reference-shell">
  <header class="global-reference-header">
    <div class="global-title-row">
      <Globe2 class="global-title-icon" :size="15" aria-hidden="true" />
      <div>
        <h1>{{ t('global.title') }}</h1>
        <p>{{ t('global.help') }}</p>
      </div>
    </div>
    <div class="global-top-actions">
      <button class="button ghost" type="button" @click="resetGlobalSettings">
        {{ t('global.resetDefaults') }}
      </button>
      <button class="button" type="button" @click="saveGlobalSettings">
        {{ t('commenter.save') }}
      </button>
    </div>
  </header>

  <div class="global-reference-grid">
    <nav class="global-subnav" aria-label="全局设置分区">
      <a href="#api-credentials">{{ t('global.section.apiCredentials') }}</a>
      <a href="#concurrency-quota">{{ t('global.section.concurrencyQuota') }}</a>
      <a href="#diff-tool">{{ t('global.section.diffTool') }}</a>
      <a href="#storage-logs">{{ t('global.section.storageLogs') }}</a>
      <a href="#about-settings">{{ t('global.section.about') }}</a>
    </nav>
    <div class="global-reference-content">
      <DiffToolSettingsPanel ref="diffSettingsPanel" variant="reference" />
    </div>
  </div>
</section>
```

- [ ] **Step 2: Wire top-level save / reset bindings**

```ts
import { ref } from 'vue';

const diffSettingsPanel = ref<{
  saveSettings: () => Promise<void>;
  resetSettings: () => void;
} | null>(null);

async function saveGlobalSettings() {
  await diffSettingsPanel.value?.saveSettings();
}

function resetGlobalSettings() {
  diffSettingsPanel.value?.resetSettings();
}
```

- [ ] **Step 3: Recompose `DiffToolSettingsPanel` for the global shell**

Restructure the template so that each settings section uses an `<section :id="…">` matching the subnav anchors. The panel must render **all five sections** so that anchor links scroll to a real target:

```vue
<template>
  <section :class="['panel', { 'global-form-panel': props.variant === 'reference' }]">
    <section id="api-credentials" class="global-section">
      <header class="global-section-header">
        <h3>{{ t('global.section.apiCredentials') }}</h3>
        <p>{{ t('global.section.apiCredentialsHelp') }}</p>
      </header>
      <div class="field field-span-2">
        <label>{{ t('commenter.diff.apiBearerToken') }}</label>
        <input v-model.trim="form.api_bearer_token" type="password" autocomplete="off">
        <p class="field-hint">{{ t('commenter.diff.hint.apiBearerToken') }}</p>
      </div>
      <div class="credentials-status-pill" data-state="unverified">
        {{ t('global.credential.notVerified') }}
      </div>
    </section>

    <section id="concurrency-quota" class="global-section">
      <header class="global-section-header">
        <h3>{{ t('global.section.concurrencyQuota') }}</h3>
        <p>{{ t('global.section.concurrencyQuotaHelp') }}</p>
      </header>
      <div class="field-grid">
        <div class="field">
          <label>{{ t('commenter.diff.globalMaxWorkers') }}</label>
          <input v-model.number="form.global_max_workers" type="number" min="1">
        </div>
        <div class="field">
          <label>{{ t('commenter.diff.apiConcurrencyLimit') }}</label>
          <input v-model.number="form.api_concurrency_limit" type="number" min="1">
        </div>
        <div class="field single-file-token-placeholder">
          <label>{{ t('global.singleFileToken.label') }}</label>
          <input type="number" disabled value="8000">
          <p class="field-hint">{{ t('global.singleFileToken.disabled') }}</p>
        </div>
      </div>
    </section>

    <section id="diff-tool" class="global-section">
      <header class="global-section-header">
        <h3>{{ t('global.section.diffTool') }}</h3>
        <p>{{ t('global.section.diffToolHelp') }}</p>
      </header>
      <div class="field">
        <label>{{ t('commenter.diff.commandTemplate') }}</label>
        <input v-model="form.command_template" class="mono">
        <p class="field-hint">{{ t('global.diff.placeholdersHelp') }}</p>
      </div>
    </section>

    <section id="storage-logs" class="global-section">
      <header class="global-section-header">
        <h3>{{ t('global.section.storageLogs') }}</h3>
        <p>{{ t('global.section.storageLogsHelp') }}</p>
      </header>
      <dl class="global-readonly-list">
        <div>
          <dt>{{ t('global.storage.databaseDir') }}</dt>
          <dd class="mono">{{ storage_database_dir }}</dd>
        </div>
        <div>
          <dt>{{ t('global.storage.logDir') }}</dt>
          <dd class="mono">{{ storage_log_dir }}</dd>
        </div>
      </dl>
    </section>

    <section id="about-settings" class="global-section">
      <header class="global-section-header">
        <h3>{{ t('global.section.about') }}</h3>
      </header>
      <dl class="global-readonly-list">
        <div>
          <dt>{{ t('global.about.version') }}</dt>
          <dd class="mono">{{ app_version }}</dd>
        </div>
        <div>
          <dt>{{ t('global.about.repository') }}</dt>
          <dd class="mono">ai-comment-orchestrator</dd>
        </div>
      </dl>
    </section>
  </section>
</template>
```

- [ ] **Step 4: Add reset/expose, storage values, and compute static fields**

```ts
import { computed, reactive, watch } from 'vue';
import { commenterStore } from '../../lib/commenterStore';
import { use_messages } from '../../locales/messages';

const { t } = use_messages();

const props = withDefaults(defineProps<{ variant?: 'panel' | 'reference' }>(), {
  variant: 'panel'
});

const form = reactive({
  command_template: 'code --diff "{before}" "{after}"',
  global_max_workers: 2,
  api_concurrency_limit: 2,
  api_bearer_token: ''
});

watch(
  () => commenterStore.state.diff_tool_settings,
  (value) => {
    if (value) {
      form.command_template = value.command_template;
    }
  },
  { immediate: true }
);

watch(
  () => commenterStore.state.app_settings,
  (value) => {
    if (value) {
      form.global_max_workers = value.global_max_workers;
      form.api_concurrency_limit = value.api_concurrency_limit;
      form.api_bearer_token = value.api_bearer_token;
    }
  },
  { immediate: true }
);

const storage_database_dir = computed(() => commenterStore.state.app_settings?.database_dir ?? '—');
const storage_log_dir = computed(() => commenterStore.state.app_settings?.log_dir ?? '—');
const app_version = computed(() => import.meta.env.VITE_APP_VERSION ?? 'dev');

async function saveSettings() {
  await commenterStore.saveDiffToolSettings({ command_template: form.command_template });
  await commenterStore.saveAppSettings({
    global_max_workers: form.global_max_workers,
    api_concurrency_limit: form.api_concurrency_limit,
    api_bearer_token: form.api_bearer_token
  });
}

function resetSettings() {
  form.command_template = 'code --diff "{before}" "{after}"';
  form.global_max_workers = 2;
  form.api_concurrency_limit = 2;
  form.api_bearer_token = '';
}

defineExpose({ saveSettings, resetSettings });
```

> If `database_dir` / `log_dir` do not yet exist on `CommenterRunSettingsView`, the computed values fall back to `—` and the section still renders. Adding those fields is out of scope for this migration.

- [ ] **Step 5: Add locale keys (Chinese-only)**

```ts
'global.title': '全局设置',
'global.help': '跨项目共享，仅本机存储，可在每个项目内被覆盖。',
'global.resetDefaults': '恢复默认',
'global.section.apiCredentials': 'API 凭证',
'global.section.apiCredentialsHelp': '所有项目共用同一组凭证；Token 只保存在本机 keychain 中。',
'global.section.concurrencyQuota': '并发配额',
'global.section.concurrencyQuotaHelp': '所有 run 共享同一池子；项目可以申请更小的子配额。',
'global.section.diffTool': 'Diff 工具',
'global.section.diffToolHelp': '审阅时打开外部 diff 的命令模板。',
'global.section.storageLogs': '存储与日志',
'global.section.storageLogsHelp': '本地数据库与日志的存放位置。',
'global.section.about': '关于',
'global.credential.notVerified': '凭证未校验 · 保存后由后端在下一次请求时验证',
'global.singleFileToken.label': '单文件最大 Token',
'global.singleFileToken.disabled': '后端暂未支持，下个版本接入',
'global.diff.placeholdersHelp': '{before} {after} 会被替换为快照文件路径',
'global.storage.databaseDir': '数据库目录',
'global.storage.logDir': '日志目录',
'global.about.version': '版本',
'global.about.repository': '仓库',
```

- [ ] **Step 6: Run settings tests**

```bash
pnpm exec tsx src/lib/settingsWorkspaceEnhancements.test.ts
pnpm exec tsx src/lib/commenterLocale.test.ts
```

Expected: both pass.

### Task 6: Style The Settings And Project Workspaces

**Files:**
* Modify: `src/styles.css`

- [ ] **Step 1: Shared header / shell styles**

```css
.project-reference-shell,
.global-reference-shell {
  min-height: 100vh;
  padding: 0;
  color: var(--aco-text);
}

.project-reference-header,
.global-reference-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 58px;
  gap: 16px;
  border-bottom: 1px solid rgba(108, 142, 164, 0.16);
  padding: 14px 20px 12px;
}

.project-title-row,
.global-title-row {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}

.project-title-row h1,
.global-title-row h1 {
  margin: 0;
  color: var(--aco-text);
  font-size: 14px;
  line-height: 1.2;
}

.project-title-row p,
.global-title-row p {
  margin: 2px 0 0;
  color: var(--aco-muted);
  font-size: 12px;
}

.project-reference-content {
  width: min(100%, 960px);
  padding: 20px 26px;
}

.global-reference-grid {
  display: grid;
  grid-template-columns: 240px minmax(0, 1fr);
  min-height: calc(100vh - 58px);
}

.global-subnav {
  display: flex;
  flex-direction: column;
  gap: 8px;
  border-right: 1px solid rgba(108, 142, 164, 0.16);
  padding: 16px 10px;
}

.global-subnav a {
  display: flex;
  align-items: center;
  min-height: 34px;
  border: 1px solid transparent;
  border-radius: 7px;
  color: var(--aco-muted);
  padding: 7px 12px;
  font-size: 13px;
  text-decoration: none;
}

.global-subnav a:hover,
.global-subnav a:focus-visible {
  border-color: var(--aco-border);
  background: var(--aco-surface-2);
  color: var(--aco-text);
}

.global-reference-content {
  width: min(100%, 960px);
  padding: 20px 26px;
}

.global-section {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 20px 0;
  border-bottom: 1px solid rgba(108, 142, 164, 0.12);
}

.global-section:last-child {
  border-bottom: 0;
}

.global-section-header h3 {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--aco-text);
}

.global-section-header p {
  margin: 2px 0 0;
  color: var(--aco-muted);
  font-size: 12px;
}

.credentials-status-pill {
  display: inline-flex;
  width: max-content;
  align-items: center;
  gap: 6px;
  border: 1px solid var(--aco-border);
  border-radius: 999px;
  background: var(--aco-surface-2);
  color: var(--aco-muted);
  padding: 4px 10px;
  font-size: 12px;
}

.credentials-status-pill[data-state='verified'] {
  color: var(--aco-green);
  border-color: rgba(52, 211, 153, 0.4);
}

.global-readonly-list {
  display: grid;
  gap: 6px;
  margin: 0;
}

.global-readonly-list > div {
  display: grid;
  grid-template-columns: 140px 1fr;
  gap: 12px;
}

.global-readonly-list dt {
  color: var(--aco-muted);
  font-size: 12px;
}

.global-readonly-list dd {
  margin: 0;
  color: var(--aco-text);
  font-size: 12px;
}

.single-file-token-placeholder input[disabled] {
  color: var(--aco-subtle);
  background: var(--aco-surface-2);
  border-color: var(--aco-border);
  cursor: not-allowed;
}
```

- [ ] **Step 2: Tighten form controls**

Update `.field input`, `.field select`, `.field textarea`, `.button`, and `.panel` to use `var(--aco-*)`, 6-8px radius, 8-10px vertical padding, no large box shadows.

- [ ] **Step 3: Add responsive rules**

```css
@media (max-width: 900px) {
  .global-reference-grid {
    grid-template-columns: 1fr;
  }

  .global-subnav {
    flex-direction: row;
    overflow-x: auto;
    border-right: 0;
    border-bottom: 1px solid rgba(108, 142, 164, 0.16);
  }
}

@media (max-width: 720px) {
  .project-reference-header,
  .global-reference-header {
    align-items: flex-start;
    flex-direction: column;
  }

  .global-top-actions {
    width: 100%;
    justify-content: flex-end;
  }
}
```

- [ ] **Step 4: Run settings source tests**

```bash
pnpm exec tsx src/lib/settingsWorkspaceEnhancements.test.ts
pnpm exec tsx src/lib/referenceStyleLayout.test.ts
```

Expected: both pass.

### Task 7: Rebuild The Run Workspace Layout With Three Wired Tabs

**Files:**
* Modify: `src/pages/CommentOrchestratorPage.vue`
* Modify: `src/components/commenter/RunDetailPanel.vue`
* Modify: `src/components/commenter/QueueRunsTable.vue`
* Modify: `src/components/commenter/ExecutionLogPanel.vue`
* Modify: `src/locales/messages.ts`

- [ ] **Step 1: Render the run shell**

In the run branch of `CommentOrchestratorPage.vue`:

```vue
<section v-else class="run-reference-shell">
  <RunHeaderStrip />
  <div class="run-current-strip">
    <span>{{ t('commenter.header.current') }}</span>
    <strong class="mono">{{ commenterStore.state.selected_run_detail?.run.current_file ?? t('commenter.idle') }}</strong>
  </div>
  <RunDetailPanel variant="reference" />
</section>
```

- [ ] **Step 2: Wire `RunDetailPanel` left rail with three tabs and content switching**

In `RunDetailPanel.vue`:

```ts
import { computed, ref, watch } from 'vue';
import { commenterStore } from '../../lib/commenterStore';
import StreamContentPanel from './StreamContentPanel.vue';
import WorkspaceTreePanel from './WorkspaceTreePanel.vue';
import QueueRunsTable from './QueueRunsTable.vue';
import ExecutionLogPanel from './ExecutionLogPanel.vue';
import { use_messages } from '../../locales/messages';
import { streamSliceKey } from '../../lib/commenterStreamSlice';

const props = withDefaults(defineProps<{ variant?: 'stacked' | 'reference' }>(), {
  variant: 'stacked'
});

const { t } = use_messages();
const active_left_tab = ref<'files' | 'runs' | 'events'>('files');

const detail = computed(() => commenterStore.state.selected_run_detail);
const run = computed(() => detail.value?.run ?? null);

const file_count = computed(() => detail.value?.jobs.length ?? 0);
const run_count = computed(() => commenterStore.state.runs.length);
const event_count = computed(() => detail.value?.events.length ?? 0);

// follow_mode / selected_file / live_slice / selected_job / job_status / error_message: keep existing logic
```

Reference template branch:

```vue
<div v-if="props.variant === 'reference'" class="run-detail-reference-grid">
  <aside class="run-left-rail">
    <nav class="run-left-tabs" aria-label="左侧视图切换">
      <button type="button" :class="{ active: active_left_tab === 'files' }" @click="active_left_tab = 'files'">
        {{ t('commenter.files') }}
        <span class="left-tab-count">{{ file_count }}</span>
      </button>
      <button type="button" :class="{ active: active_left_tab === 'runs' }" @click="active_left_tab = 'runs'">
        {{ t('commenter.runs') }}
        <span class="left-tab-count">{{ run_count }}</span>
      </button>
      <button type="button" :class="{ active: active_left_tab === 'events' }" @click="active_left_tab = 'events'">
        {{ t('commenter.events') }}
        <span class="left-tab-count">{{ event_count }}</span>
      </button>
    </nav>
    <div class="run-left-body">
      <WorkspaceTreePanel v-if="active_left_tab === 'files'" @select-file="onTreeSelect" />
      <QueueRunsTable v-if="active_left_tab === 'runs'" variant="rail" />
      <ExecutionLogPanel v-if="active_left_tab === 'events'" variant="rail" />
    </div>
  </aside>
  <main class="run-stream-rail">
    <StreamContentPanel
      :mode="follow_mode"
      :run_key="run?.run_key ?? null"
      :relative_path="selected_file"
      :live_text="live_slice?.text ?? ''"
      :status="job_status"
      :error_message="error_message"
    />
  </main>
</div>
```

Keep the original stacked-grid template under `v-else` so existing callers stay compatible.

- [ ] **Step 3: Add `variant: 'panel' | 'rail'` props to `QueueRunsTable` and `ExecutionLogPanel`**

The `'rail'` variant collapses headers, hides the enqueue form (queue) or tones down the title (events), and switches to compact row padding. The default `'panel'` variant keeps the existing renderings to avoid breakage in any other consumer.

- [ ] **Step 4: Add minimal labels (Chinese-only)**

```ts
'commenter.files': '文件',
'commenter.runs': '运行',
'commenter.events': '事件',
'commenter.header.current': '当前',
```

- [ ] **Step 5: Run route and run layout tests**

```bash
pnpm exec tsx src/lib/commenterRoute.test.ts
pnpm exec tsx src/lib/referenceStyleLayout.test.ts
```

Expected: both pass after `run-reference-shell` and `active_left_tab` exist.

### Task 8: Convert RunHeaderStrip Into The Reference RunBar

**Files:**
* Modify: `src/components/commenter/RunHeaderStrip.vue`
* Modify: `src/locales/messages.ts`
* Modify: `src/styles.css`

- [ ] **Step 1: Add elapsed and throughput computed values, plus degradation flags**

```ts
function format_duration(ms: number): string {
  if (ms <= 0) {
    return '0s';
  }
  const total_seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(total_seconds / 60);
  const seconds = total_seconds % 60;
  return minutes > 0 ? `${minutes}m ${seconds}s` : `${seconds}s`;
}

const elapsed_label = computed(() => {
  if (!run.value?.started_at) {
    return '0s';
  }
  const end = run.value.finished_at ?? Date.now();
  return format_duration(end - run.value.started_at);
});

const throughput_label = computed(() => {
  if (!run.value?.started_at) {
    return '0';
  }
  const end = run.value.finished_at ?? Date.now();
  const minutes = Math.max((end - run.value.started_at) / 60000, 0.01);
  return (run.value.completed_jobs / minutes).toFixed(1);
});

// Degradation flags — these blocks render only when the runtime exposes the data.
const show_token_block = computed(() => false);   // backend has no token usage; keep flag for future wiring
const show_ttft_chip = computed(() => false);     // backend has no TTFT yet
```

- [ ] **Step 2: Replace header classes and remove all decorative gradients**

```vue
<template>
  <header v-if="run" class="runbar" :data-status="run.status">
    <div class="runbar-identity">
      <span class="runbar-project mono">{{ run.profile_key }}</span>
      <span class="status-badge runbar-status">{{ run_status_label(run.status) }}</span>
      <strong class="runbar-key mono">{{ run.run_key }}</strong>
      <span class="runbar-mode">{{ run.run_mode }}</span>
    </div>

    <div class="runbar-progress">
      <strong>{{ run.completed_jobs }} / {{ run.total_jobs }}</strong>
      <span>{{ progress }}%</span>
      <div class="runbar-progress-track"><span :style="{ width: `${progress}%` }" /></div>
    </div>

    <div class="runbar-metrics">
      <span>
        {{ t('commenter.header.elapsed') }}
        <strong>{{ elapsed_label }}</strong>
      </span>
      <span>
        {{ t('commenter.header.throughput') }}
        <strong>{{ throughput_label }}</strong>
        {{ t('commenter.header.filesPerMinute') }}
      </span>
      <span v-if="show_token_block" class="runbar-tokens">
        <!-- intentionally hidden until backend exposes token usage -->
      </span>
      <span v-if="show_ttft_chip" class="runbar-ttft">
        <!-- intentionally hidden until backend exposes TTFT -->
      </span>
    </div>

    <div class="runbar-issues">
      <span class="runbar-chip runbar-chip--review">{{ run.review_needed_jobs }} {{ t('commenter.review') }}</span>
      <span class="runbar-chip runbar-chip--failed">{{ run.failed_jobs }} {{ t('status.failed') }}</span>
      <span class="runbar-chip runbar-chip--done">{{ run.completed_jobs }} {{ t('status.completed') }}</span>
      <span class="runbar-chip runbar-chip--skipped">{{ run.skipped_jobs }} {{ t('status.skipped') }}</span>
    </div>

    <div class="runbar-actions">
      <button v-if="can_pause" type="button" :aria-label="t('commenter.header.pause')" @click="onPause">
        <Pause :size="14" />
        <span>{{ t('commenter.header.pause') }}</span>
      </button>
      <button v-if="can_resume" type="button" :aria-label="t('commenter.header.resume')" @click="onResume">
        <Play :size="14" />
        <span>{{ t('commenter.header.resume') }}</span>
      </button>
      <button v-if="can_cancel" type="button" class="runbar-cancel" :aria-label="t('commenter.header.cancel')" @click="onCancel">
        <X :size="14" />
        <span>{{ t('commenter.header.cancel') }}</span>
      </button>
    </div>
  </header>
</template>
```

- [ ] **Step 3: Replace scoped styles — flat backgrounds only**

```css
.runbar {
  display: grid;
  grid-template-columns: minmax(0, 1.1fr) minmax(220px, 1fr) minmax(0, 1fr) auto auto;
  gap: 12px;
  align-items: center;
  padding: 10px 16px;
  background: var(--aco-surface-1);
  border-bottom: 1px solid var(--aco-border);
}

.runbar-progress-track {
  height: 6px;
  border-radius: 999px;
  overflow: hidden;
  background: var(--aco-surface-3);
}

.runbar-progress-track > span {
  display: block;
  height: 100%;
  background: var(--aco-green);
}

.runbar-cancel {
  background: rgba(239, 90, 111, 0.08);
  border-color: rgba(239, 90, 111, 0.32);
}
```

The decorative `linear-gradient(135deg, rgba(52, 211, 153, 0.08), rgba(14, 116, 144, 0.1))` and the multi-stop progress gradient must not survive this rewrite.

- [ ] **Step 4: Add metric labels (Chinese-only)**

```ts
'commenter.header.elapsed': '已用',
'commenter.header.throughput': '吞吐',
'commenter.header.filesPerMinute': '文件/分',
'commenter.header.workers': 'workers',
'commenter.header.api': 'api',
'status.skipped': '跳过',
```

- [ ] **Step 5: Run RunBar test**

```bash
pnpm exec tsx src/lib/commenterRunHeader.test.ts
pnpm exec tsx src/lib/referenceStyleLayout.test.ts
```

Expected: pass.

### Task 9: Polish File Tree, Stream Panel, And Events Rail

**Files:**
* Modify: `src/components/commenter/WorkspaceTreePanel.vue`
* Modify: `src/components/commenter/StreamContentPanel.vue`
* Modify: `src/components/commenter/ExecutionLogPanel.vue`
* Modify: `src/styles.css`
* Modify: `src/locales/messages.ts`

- [ ] **Step 1: Add stream tabs and rich meta to `StreamContentPanel`**

Add the tab ref and meta computeds:

```ts
const active_tab = ref<'diff' | 'stream' | 'original' | 'events'>('stream');

const language_label = computed(() => {
  const path = props.relative_path ?? '';
  const ext = path.split('.').pop()?.toLowerCase() ?? '';
  const map: Record<string, string> = {
    ts: 'TypeScript', tsx: 'TSX', js: 'JavaScript', jsx: 'JSX',
    vue: 'Vue', go: 'Go', rs: 'Rust', py: 'Python', java: 'Java',
    css: 'CSS', html: 'HTML', md: 'Markdown', json: 'JSON', yaml: 'YAML', yml: 'YAML',
    sql: 'SQL', sh: 'Shell', toml: 'TOML', proto: 'Protobuf'
  };
  return map[ext] ?? ext.toUpperCase() || '';
});

const size_kb_label = computed(() => {
  const bytes = new TextEncoder().encode(display_text.value).length;
  return bytes > 0 ? `${(bytes / 1024).toFixed(1)} KB` : '';
});

const line_count_label = computed(() => {
  if (!display_text.value) return '';
  return `${display_text.value.split('\n').length} 行`;
});

const chunk_count_label = computed(() => {
  const slice = props.live_text ? Math.max(1, Math.ceil(props.live_text.length / 64)) : 0;
  return slice > 0 ? `${slice} chunks` : '';
});
```

> `chunk_count_label` is a heuristic until the live-stream slice exposes a real chunk counter; the heuristic is acceptable because the screenshot value is informational, not load-bearing for behavior.

Header template:

```vue
<header class="stream-header">
  <div class="stream-header-main">
    <span class="stream-path mono">{{ relative_path ?? t('commenter.stream.idle') }}</span>
    <span v-if="language_label" class="stream-lang">{{ language_label }}</span>
    <span v-if="size_kb_label" class="stream-size">{{ size_kb_label }}</span>
    <span v-if="line_count_label" class="stream-lines">{{ line_count_label }}</span>
  </div>
  <span :class="badge_class">{{ badge_label }}</span>
</header>

<nav class="stream-tabs" aria-label="流视图切换">
  <button type="button" :class="{ active: active_tab === 'diff' }" @click="active_tab = 'diff'">Diff</button>
  <button type="button" :class="{ active: active_tab === 'stream' }" @click="active_tab = 'stream'">{{ t('commenter.stream.response') }}</button>
  <button type="button" :class="{ active: active_tab === 'original' }" @click="active_tab = 'original'">{{ t('commenter.stream.original') }}</button>
  <button type="button" :class="{ active: active_tab === 'events' }" @click="active_tab = 'events'">{{ t('commenter.stream.fileEvents') }}</button>
</nav>

<div class="stream-meta">
  <span>UTF-8</span>
  <span>LF</span>
  <span v-if="chunk_count_label">{{ chunk_count_label }}</span>
</div>
```

Only the `stream` tab renders the streaming `<pre>`; the others render `<div class="empty-state">{{ t('commenter.stream.unavailableView') }}</div>` until they have real data sources.

- [ ] **Step 2: Add locale keys**

```ts
'commenter.stream.response': '流式响应',
'commenter.stream.original': '原文',
'commenter.stream.fileEvents': '文件事件',
'commenter.stream.unavailableView': '该视图暂不适用于当前文件。',
```

- [ ] **Step 3: Compact tree, events, and stream styles**

```css
.workspace-tree-panel,
.execution-log-panel,
.stream-content-panel {
  border-radius: 0;
  background: var(--aco-surface-1);
  border-color: var(--aco-border);
}

.tree li {
  padding: 2px 6px;
}

.stream-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  border-bottom: 1px solid var(--aco-border);
  padding: 8px 14px;
  font-size: 12px;
}

.stream-header-main {
  display: flex;
  align-items: baseline;
  gap: 12px;
  min-width: 0;
}

.stream-path {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--aco-text);
}

.stream-lang,
.stream-size,
.stream-lines {
  color: var(--aco-muted);
  font-size: 12px;
}

.stream-tabs {
  display: flex;
  gap: 6px;
  border-bottom: 1px solid var(--aco-border);
  padding: 4px 8px;
}

.stream-tabs button {
  border: 1px solid transparent;
  border-radius: 6px;
  background: transparent;
  color: var(--aco-muted);
  cursor: pointer;
  padding: 4px 10px;
  font-size: 12px;
}

.stream-tabs button.active {
  border-color: var(--aco-border);
  background: var(--aco-surface-2);
  color: var(--aco-text);
}

.stream-meta {
  display: flex;
  gap: 12px;
  border-top: 1px solid var(--aco-border);
  padding: 6px 14px;
  color: var(--aco-muted);
  font-size: 11px;
}
```

- [ ] **Step 4: Run stream tests**

```bash
pnpm exec tsx src/lib/commenterStreamPanel.test.ts
pnpm exec tsx src/lib/commenterWorkspaceTree.test.ts
pnpm exec tsx src/lib/commenterExecutionLog.test.ts
```

Expected: all pass.

### Task 10: Final Responsive And Visual Verification

**Files:**
* Modify: `src/styles.css`

- [ ] **Step 1: Add run workspace CSS**

```css
.run-reference-shell {
  display: flex;
  min-height: 100vh;
  flex-direction: column;
  background: var(--aco-bg);
}

.run-current-strip {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 32px;
  border-bottom: 1px solid rgba(108, 142, 164, 0.16);
  background: var(--aco-surface-2);
  color: var(--aco-muted);
  padding: 6px 18px;
  font-size: 12px;
}

.run-current-strip strong {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--aco-green);
}

.run-detail-reference-grid {
  display: grid;
  grid-template-columns: 360px minmax(0, 1fr);
  min-height: 0;
  flex: 1;
}

.run-left-rail {
  display: flex;
  flex-direction: column;
  min-width: 0;
  border-right: 1px solid var(--aco-border);
  background: var(--aco-surface-1);
}

.run-left-tabs {
  display: flex;
  gap: 4px;
  border-bottom: 1px solid var(--aco-border);
  padding: 6px 8px;
}

.run-left-tabs button {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--aco-muted);
  cursor: pointer;
  padding: 4px 8px;
  font-size: 12px;
}

.run-left-tabs button.active {
  background: var(--aco-surface-2);
  color: var(--aco-text);
}

.left-tab-count {
  border-radius: 999px;
  background: var(--aco-surface-3);
  color: var(--aco-muted);
  padding: 0 6px;
  font-size: 11px;
}

.run-left-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.run-stream-rail {
  min-width: 0;
  background: var(--aco-bg);
}
```

- [ ] **Step 2: Add mobile collapse rules**

```css
@media (max-width: 1080px) {
  .app-shell,
  .app-shell--reference {
    grid-template-columns: 1fr;
  }

  .sidebar {
    min-height: auto;
    border-right: 0;
    border-bottom: 1px solid var(--aco-border);
  }

  .run-detail-reference-grid {
    grid-template-columns: 1fr;
  }
}
```

- [ ] **Step 3: Run the smoke suite**

```bash
pnpm run smoke
```

Expected: all smoke tests pass, including the new `commenterLocale.test.ts`, `referenceStyleLayout.test.ts`, and updated `commenterRoute.test.ts`.

- [ ] **Step 4: Run the production build**

```bash
pnpm run build
```

Expected: `vue-tsc --noEmit` and `vite build` complete successfully — confirms the locale-storage removal and `LocaleCode` narrowing did not break any importer.

- [ ] **Step 5: Capture screenshots**

```bash
pnpm dev --host 127.0.0.1 --port 4173
```

```bash
npx playwright screenshot --viewport-size=1440,920 http://127.0.0.1:4173/settings artifacts/reference-style-project-desktop.png
npx playwright screenshot --viewport-size=1440,920 http://127.0.0.1:4173/global   artifacts/reference-style-global-desktop.png
npx playwright screenshot --viewport-size=1440,920 http://127.0.0.1:4173/workspace artifacts/reference-style-workspace-desktop.png
npx playwright screenshot --viewport-size=375,812  http://127.0.0.1:4173/settings artifacts/reference-style-project-mobile.png
npx playwright screenshot --viewport-size=375,812  http://127.0.0.1:4173/global   artifacts/reference-style-global-mobile.png
npx playwright screenshot --viewport-size=375,812  http://127.0.0.1:4173/workspace artifacts/reference-style-workspace-mobile.png
```

Expected: screenshots show three distinct top-level shells, no English locale text, no decorative gradients, populated subnav anchors with their target sections, three tab variants visible in the run rail, and stream meta showing language / KB / lines / chunks. Compare side-by-side against the two reference screenshots in `docs/`.

## Self-Review Checklist

* English locale and locale switch are removed end-to-end (messages, App.vue, storage, type union, tests).
* Three top-level routes exist: `/settings`, `/workspace`, `/global`.
* Project Profiles lives under `/settings`, not under the Global subnav.
* Sidebar shows three nav entries with badges and a status card with current/max concurrency dot.
* RunBar has no decorative gradients, and unbacked metrics (token, TTFT) are hidden via `v-if` flags rather than rendered with fake data.
* Global Settings subnav anchors all resolve to real `<section :id="…">` content (including Storage & Logs and About).
* `RunDetailPanel` left rail switches content among Files / Runs / Events using `active_left_tab`.
* `StreamContentPanel` exposes language, KB, lines, and chunk meta.
* `single-file-token-placeholder` and `credentials-status-pill` are explicit placeholders, with copy that names the missing backend support.
* Tests cover route additions, locale removal, RunBar gradient absence, run-rail tab wiring, and stream meta computeds.
* Verification includes source tests, smoke, build, and screenshots at desktop and mobile viewports for all three routes.
