import assert from 'node:assert/strict';
import fs from 'node:fs';

const settings_page = new URL('../pages/CommentOrchestratorPage.vue', import.meta.url);
const profiles_panel = new URL('../components/commenter/ProjectProfilesPanel.vue', import.meta.url);
const diff_panel = new URL('../components/commenter/DiffToolSettingsPanel.vue', import.meta.url);
const messages_file = new URL('../locales/messages.ts', import.meta.url);

const settings_page_source = fs.readFileSync(settings_page, 'utf8');
assert.match(settings_page_source, /global-reference-shell/, 'global settings shell should exist');
assert.match(settings_page_source, /project-reference-shell/, 'project config shell should exist');
assert.match(settings_page_source, /global-subnav/, 'global settings should expose a subnav');
assert.match(settings_page_source, /global-top-actions/, 'global settings should expose reset/save actions');
assert.match(settings_page_source, /workspaceMode === 'project'/, 'project mode branch must remain');
assert.match(settings_page_source, /workspaceMode === 'global'/, 'global mode branch must exist');

const profiles_panel_source = fs.readFileSync(profiles_panel, 'utf8');
assert.match(profiles_panel_source, /profile-form-grid/, 'project profile fields should keep the established form grid');
for (const field of ['api_base_url', 'api_model', 'request_timeout_secs']) {
  assert.match(profiles_panel_source, new RegExp(field), `${field} should remain in project profile settings`);
}

const diff_panel_source = fs.readFileSync(diff_panel, 'utf8');
assert.match(diff_panel_source, /defineExpose/, 'global settings panel should expose save/reset methods');
assert.match(diff_panel_source, /api_bearer_token/, 'global API token should stay in app settings');
assert.match(diff_panel_source, /credentials-status-pill/, 'global settings should render a verified-credential placeholder pill');
assert.match(diff_panel_source, /single-file-token-placeholder/, 'global settings should render the disabled single-file token placeholder');

const messages_source = fs.readFileSync(messages_file, 'utf8');
for (const key of [
  'global.title',
  'global.help',
  'global.section.apiCredentials',
  'global.section.concurrencyQuota',
  'global.section.diffTool',
  'global.section.storageLogs',
  'global.section.about',
  'global.resetDefaults',
  'global.credential.notVerified',
  'global.singleFileToken.disabled',
  'global.storage.dataRoot',
  'global.storage.artifactsRoot',
  'global.storage.databaseFile',
  'global.about.version'
]) {
  assert.equal(messages_source.includes(`'${key}'`), true, `${key} should exist in locale messages`);
}

console.log('settings workspace enhancements PASSED');
