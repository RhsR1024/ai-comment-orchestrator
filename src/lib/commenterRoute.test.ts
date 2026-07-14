import assert from 'node:assert/strict';
import fs from 'node:fs';

import router from '../router/index';

const root_route = router.getRoutes().find((item) => item.path === '/');
assert.equal(root_route?.redirect, '/settings', 'root should redirect to settings');

const settings_route = router.getRoutes().find((item) => item.path === '/settings');
assert.ok(settings_route, 'settings route should exist');

const workspace_route = router.getRoutes().find((item) => item.path === '/workspace');
assert.ok(workspace_route, 'workspace route should exist');

const global_route = router.getRoutes().find((item) => item.path === '/global');
assert.ok(global_route, 'global route should exist');

const tools_route = router.getRoutes().find((item) => item.path === '/tools');
assert.equal(tools_route?.redirect, '/settings', 'tools should redirect to settings');

const legacy_commenter_route = router.getRoutes().find((item) => item.path === '/tools/comment-orchestrator');
assert.equal(
  legacy_commenter_route?.redirect,
  '/settings',
  'legacy comment orchestrator route should redirect to settings'
);

assert.equal(
  router.getRoutes().some((item) => item.path === '/tools/appliance-ssh'),
  false,
  'appliance ssh route should be removed'
);
assert.equal(
  router.getRoutes().some((item) => item.path === '/tools/framework-password'),
  false,
  'framework password route should be removed'
);

for (const path of ['/console', '/tasks', '/history']) {
  const route = router.getRoutes().find((item) => item.path === path);
  assert.equal(route?.redirect, '/workspace', `${path} should redirect to the run workspace`);
}

const router_source = fs.readFileSync(new URL('../router/index.ts', import.meta.url), 'utf8');
assert.equal(
  router_source.includes('SimplePlaceholderPage'),
  false,
  'sidebar routes should no longer point to placeholder pages'
);

for (const path of ["'/settings'", "'/workspace'", "'/global'"]) {
  assert.match(router_source, new RegExp(path), `${path} route should exist`);
}

assert.match(router_source, /workspaceMode: 'project'/, 'project mode should be wired to /settings');
assert.match(router_source, /workspaceMode: 'run'/, 'run mode should be wired to /workspace');
assert.match(router_source, /workspaceMode: 'review'/, 'review mode should be wired to /review');
assert.match(router_source, /workspaceMode: 'global'/, 'global mode should be wired to /global');

const sidebar_source = fs.readFileSync(new URL('../components/Sidebar.vue', import.meta.url), 'utf8');
for (const label of ['nav.console', 'nav.tasks', 'nav.history']) {
  assert.equal(sidebar_source.includes(label), false, `${label} should be removed from the sidebar`);
}
assert.match(sidebar_source, /nav\.projectConfig/, 'sidebar should expose project configuration');
assert.match(sidebar_source, /nav\.runWorkspace/, 'sidebar should expose the run workspace');
assert.match(sidebar_source, /nav\.globalSettings/, 'sidebar should expose global settings');

console.log('commenter route PASSED');
