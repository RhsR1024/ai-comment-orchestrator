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
const queue_runs_table = fs.readFileSync(
  new URL('../components/commenter/QueueRunsTable.vue', import.meta.url),
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
assert.match(run_detail, /'files'.*'runs'/s, 'run detail must expose files and runs tabs');
assert.match(run_detail, /v-if="active_left_tab === 'runs'"/, 'runs tab must render its content');
assert.doesNotMatch(run_detail, /active_left_tab === 'events'/, 'events tab must be removed for performance');
assert.doesNotMatch(run_header, /linear-gradient\(135deg/, 'run header must not keep decorative gradient');
assert.match(queue_runs_table, /queue-rail-form/, 'run workspace rail must expose a compact profile picker');
assert.match(queue_runs_table, /enqueueAndStartRun/, 'run workspace rail must offer one-click enqueue and start');
assert.match(
  queue_runs_table,
  /t\('commenter\.queue\.emptyProfiles'\)/,
  'run workspace rail must guide first-time users back to project configuration'
);
assert.match(
  queue_runs_table,
  /queue-rail-field/,
  'run workspace rail controls must use labelled dark fields instead of native white controls'
);
assert.match(
  queue_runs_table,
  /queue-rail-control/,
  'run workspace rail select and input controls must share the dark control treatment'
);
assert.match(
  styles,
  /\.queue-rail-control/,
  'run workspace rail dark control styles must live in shared styles'
);

console.log('reference style layout PASSED');
