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
assert.match(source, /commenterApi\.getOriginalText/, 'stream panel should fetch the original snapshot on demand');
assert.match(source, /stream-tabs/, 'stream panel should expose reference-style stream tabs');
assert.match(source, /stream-meta/, 'stream panel should expose file and stream metadata');
assert.match(source, /size_kb_label/, 'stream panel should derive file size in KB');
assert.match(source, /line_count_label/, 'stream panel should derive line count');
assert.match(source, /chunk_count_label/, 'stream panel should derive chunk count');
assert.match(source, /language_label/, 'stream panel should derive language tag from path');
assert.match(source, /is_streaming_response/, 'request details should detect in-flight stream responses');
assert.match(
  source,
  /commenter\.request\.streaming/,
  'request details should label streaming responses instead of showing idle request state'
);
assert.match(source, /streamLastChunkAt/, 'request details should receive live stream chunk timing');
assert.match(source, /request_detail_events/, 'request details should render a merged timeline with live stream status');
assert.match(source, /active_tab === 'diff'/, 'diff tab should render a content branch');
assert.match(source, /active_tab === 'original'/, 'original tab should render a content branch');
assert.match(source, /diff-preview-grid/, 'diff tab should show original and candidate content side by side');
assert.match(styles, /prefers-reduced-motion/, 'shared styles should respect reduced motion');

console.log('commenter stream panel PASSED');
