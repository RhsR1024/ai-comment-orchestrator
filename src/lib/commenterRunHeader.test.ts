import assert from 'node:assert/strict';
import fs from 'node:fs';

const source = fs.readFileSync(
  new URL('../components/commenter/RunHeaderStrip.vue', import.meta.url),
  'utf8'
);

assert.match(source, /pauseRun/, 'header should call pauseRun on pause click');
assert.match(source, /resumeRun/, 'header should call resumeRun on resume click');
assert.match(source, /cancelRun/, 'header should call cancelRun on cancel click');
assert.match(source, /run_status_label/, 'header should render run status label');
assert.match(source, /selected_run_detail/, 'header should read selected_run_detail from store');

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
