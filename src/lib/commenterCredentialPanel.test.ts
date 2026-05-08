import assert from 'node:assert/strict';
import fs from 'node:fs';

const profile_panel = new URL('../components/commenter/ProjectProfilesPanel.vue', import.meta.url);
const settings_panel = new URL('../components/commenter/DiffToolSettingsPanel.vue', import.meta.url);
const messages_file = new URL('../locales/messages.ts', import.meta.url);

const profile_source = fs.readFileSync(profile_panel, 'utf8');
assert.doesNotMatch(
  profile_source,
  /credential_profile_key/,
  'project profile panel should not expose a credential key field'
);
assert.doesNotMatch(
  profile_source,
  /api_bearer_token/,
  'project profile panel should not expose a project-scoped API bearer token field'
);

const settings_source = fs.readFileSync(settings_panel, 'utf8');
assert.match(
  settings_source,
  /form\.api_bearer_token/,
  'global settings panel should expose the shared API bearer token field'
);

const messages_source = fs.readFileSync(messages_file, 'utf8');
assert.match(
  messages_source,
  /commenter\.diff\.apiBearerToken/,
  'messages should define the global API bearer token label'
);
assert.doesNotMatch(
  messages_source,
  /commenter\.profile\.hint\.credentialKeyEnvOnly/,
  'messages should not keep env-var credential key guidance'
);

console.log('commenter credential panel PASSED');
