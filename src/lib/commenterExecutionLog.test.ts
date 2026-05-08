import assert from 'node:assert/strict';
import fs from 'node:fs';

const queue_panel = new URL('../components/commenter/QueueRunsTable.vue', import.meta.url);
const log_panel = new URL('../components/commenter/ExecutionLogPanel.vue', import.meta.url);
const store_file = new URL('./commenterStore.ts', import.meta.url);
const tauri_file = new URL('./tauri.ts', import.meta.url);
const types_file = new URL('./commenterTypes.ts', import.meta.url);

const queue_source = fs.readFileSync(queue_panel, 'utf8');
assert.match(queue_source, /Trash2/, 'queue rows should expose a delete icon action');
assert.match(queue_source, /deleteRun/, 'queue rows should call the store delete action');

const log_source = fs.readFileSync(log_panel, 'utf8');
assert.match(log_source, /buildFileLogEntries/, 'execution log should derive entries via buildFileLogEntries');
assert.match(log_source, /select-file/, 'execution log should emit select-file when a row is clicked');
assert.match(log_source, /toggle/, 'execution log should toggle phase expansion');

const store_source = fs.readFileSync(store_file, 'utf8');
assert.match(store_source, /execution_logs/, 'store should retain live execution log entries');
assert.match(store_source, /subscribeCommenterEvents/, 'store should subscribe to backend execution events');

const tauri_source = fs.readFileSync(tauri_file, 'utf8');
assert.match(tauri_source, /commenter_delete_run/, 'Tauri API should expose run deletion');
assert.match(tauri_source, /commenter:\/\/state/, 'Tauri API should listen to commenter state events');

const types_source = fs.readFileSync(types_file, 'utf8');
for (const kind of ['request_started', 'stream_chunk', 'model_response_completed']) {
  assert.equal(types_source.includes(`'${kind}'`), true, `${kind} should be typed as an event kind`);
}

console.log('commenter execution log PASSED');
