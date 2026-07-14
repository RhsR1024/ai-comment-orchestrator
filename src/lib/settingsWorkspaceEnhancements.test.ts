import assert from 'node:assert/strict';
import fs from 'node:fs';

const settings_page = new URL('../pages/CommentOrchestratorPage.vue', import.meta.url);
const profiles_panel = new URL('../components/commenter/ProjectProfilesPanel.vue', import.meta.url);
const diff_panel = new URL('../components/commenter/DiffToolSettingsPanel.vue', import.meta.url);
const messages_file = new URL('../locales/messages.ts', import.meta.url);
const styles_file = new URL('../styles.css', import.meta.url);

const settings_page_source = fs.readFileSync(settings_page, 'utf8');
assert.match(settings_page_source, /global-reference-shell/, 'global settings shell should exist');
assert.match(settings_page_source, /project-reference-shell/, 'project config shell should exist');
assert.match(settings_page_source, /global-subnav/, 'global settings should expose a subnav');
assert.match(settings_page_source, /activeGlobalSection/, 'global settings navigation should track the selected section');
assert.match(settings_page_source, /:active-section="activeGlobalSection"/, 'selected navigation section should control the settings panel');
assert.doesNotMatch(settings_page_source, /href="#api-credentials"/, 'global settings navigation should not be passive anchor links');
assert.match(settings_page_source, /global-top-actions/, 'global settings should expose reset/save actions');
assert.match(settings_page_source, /workspaceMode === 'project'/, 'project mode branch must remain');
assert.match(settings_page_source, /workspaceMode === 'global'/, 'global mode branch must exist');

const profiles_panel_source = fs.readFileSync(profiles_panel, 'utf8');
assert.match(profiles_panel_source, /profile-form-grid/, 'project profile fields should keep the established form grid');
for (const field of ['api_base_url', 'api_model', 'request_timeout_secs']) {
  assert.match(profiles_panel_source, new RegExp(field), `${field} should remain in project profile settings`);
}
assert.match(profiles_panel_source, /save_state === 'saving'/, 'project profile save should expose progress feedback');
assert.match(profiles_panel_source, /profile-save-feedback--ok/, 'project profile save should expose success feedback');
assert.match(profiles_panel_source, /profile-save-feedback--error/, 'project profile save should expose failure feedback');

const diff_panel_source = fs.readFileSync(diff_panel, 'utf8');
assert.match(diff_panel_source, /defineExpose/, 'global settings panel should expose save/reset methods');
assert.match(diff_panel_source, /api_bearer_token/, 'global API token should stay in app settings');
assert.match(diff_panel_source, /credentials-status-pill/, 'global settings should render a verified-credential placeholder pill');
assert.match(diff_panel_source, /props\.activeSection === 'api-credentials'/, 'global settings panel should filter the API section');
assert.match(diff_panel_source, /props\.activeSection === 'about-settings'/, 'global settings panel should filter the About section');
assert.doesNotMatch(diff_panel_source, /single-file-token-placeholder/, 'unsupported single-file token setting should be absent');

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
  'global.storage.dataRoot',
  'global.storage.artifactsRoot',
  'global.storage.databaseFile',
  'global.about.version'
]) {
  assert.equal(messages_source.includes(`'${key}'`), true, `${key} should exist in locale messages`);
}

assert.equal(messages_source.includes('global.singleFileToken'), false, 'obsolete single-file token messages should be removed');
assert.equal(
  fs.readFileSync(styles_file, 'utf8').includes('single-file-token-placeholder'),
  false,
  'obsolete single-file token styles should be removed'
);

console.log('settings workspace enhancements PASSED');
