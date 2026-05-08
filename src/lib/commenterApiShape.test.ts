import assert from 'node:assert/strict';
import fs from 'node:fs';

import { commenterApi, pickProjectRootPath, subscribeCommenterEvents } from './tauri';

assert.equal(typeof commenterApi.enqueueRun, 'function');
assert.equal(typeof commenterApi.pauseRun, 'function');
assert.equal(typeof commenterApi.deleteRun, 'function');
assert.equal(typeof commenterApi.rollbackRun, 'function');
assert.equal(typeof pickProjectRootPath, 'function');
assert.equal(typeof subscribeCommenterEvents, 'function');

const store_source = fs.readFileSync(new URL('./commenterStore.ts', import.meta.url), 'utf8');
assert.match(store_source, /applyEventToStreamSlices/, 'store should pipe events through stream slice reducer');
assert.match(store_source, /live_streams: new Map\(\)/, 'store should initialize live_streams');
assert.match(store_source, /state\.live_streams = new Map\(\)/, 'selectRun should clear live_streams');

console.log('commenterApi shape PASSED');
