import assert from 'node:assert/strict';
import fs from 'node:fs';

const source = fs.readFileSync(
  new URL('../components/commenter/WorkspaceTreePanel.vue', import.meta.url),
  'utf8'
);

assert.match(source, /commenterApi\.listDir/, 'tree should call commenterApi.listDir');
assert.match(source, /current_file/, 'tree should reference current_file for auto-expand');
assert.match(source, /active_paths/, 'tree should track all active run paths for auto-expand');
assert.match(source, /expandActivePaths/, 'tree should expand directories for active running files');
assert.match(source, /queued_paths/, 'tree should highlight queued paths');
assert.match(source, /job_status_by_path/, 'tree should pass per-file job statuses into render nodes');
assert.match(source, /select-file/, 'tree should emit select-file');

const node_source = fs.readFileSync(
  new URL('../components/commenter/WorkspaceTreeNode.vue', import.meta.url),
  'utf8'
);
assert.match(node_source, /Loader2/, 'current or running files should render a spinner icon');
assert.match(node_source, /activePaths/, 'tree nodes should receive all active paths');
assert.match(node_source, /scrollIntoView/, 'current tree node should scroll into view when it becomes active');
assert.match(node_source, /tree-node-status--active/, 'running file status should have an active style hook');
assert.match(node_source, /tree-node--active/, 'running files should have an active row style hook');

console.log('commenter workspace tree PASSED');
