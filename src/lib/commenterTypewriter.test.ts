import assert from 'node:assert/strict';

import { advanceTypewriterText } from './commenterTypewriter';

assert.equal(advanceTypewriterText('', '中文'), '中', 'small deltas should reveal one character');
assert.equal(advanceTypewriterText('中', '中文'), '中文', 'typewriter should preserve exact ordering');
assert.equal(
  advanceTypewriterText('old', 'replacement'),
  'replacement',
  'non-prefix replacements should reset without mixing files'
);
assert.ok(
  advanceTypewriterText('', 'x'.repeat(2000)).length <= 16,
  'large upstream deltas should remain bounded per frame'
);

console.log('commenter typewriter PASSED');
