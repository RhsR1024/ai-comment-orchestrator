import assert from 'node:assert/strict';

import {
  DEFAULT_COMMENT_PROMPT_TEMPLATE,
  createDefaultCommenterProjectProfileDraft
} from './commenterProfileDefaults';

const draft = createDefaultCommenterProjectProfileDraft();

assert.equal(draft.project_key, 'demo-project');
assert.equal(draft.root_path, '');
assert.equal(
  draft.prompt_template,
  DEFAULT_COMMENT_PROMPT_TEMPLATE,
  'project profile draft should use the shared prompt template'
);
assert.match(DEFAULT_COMMENT_PROMPT_TEMPLATE, /^---\n/);
assert.match(DEFAULT_COMMENT_PROMPT_TEMPLATE, /# 中文注释规范（Code Comment Style）/);
assert.match(DEFAULT_COMMENT_PROMPT_TEMPLATE, /如果是其他语言，注释的细则方式不变/);
assert.equal('credential_profile_key' in draft.settings, false);
assert.equal(draft.settings.api_base_url, 'https://unvcoding.copilot.qq.com');
assert.equal(draft.settings.api_model, 'glm-5.1');
assert.equal('api_bearer_token' in draft.settings, false);
assert.equal(draft.settings.request_timeout_secs, 600);
assert.equal(draft.settings.default_max_files, 0, 'new profiles should process the whole project');

console.log('commenter profile defaults PASSED');
