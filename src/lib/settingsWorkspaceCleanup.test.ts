import assert from 'node:assert/strict';
import fs from 'node:fs';

const tools_hub_page = new URL('../pages/ToolsHubPage.vue', import.meta.url);
const tool_placeholder_page = new URL('../pages/ToolPlaceholderPage.vue', import.meta.url);
const settings_page = new URL('../pages/CommentOrchestratorPage.vue', import.meta.url);
const messages_file = new URL('../locales/messages.ts', import.meta.url);
const styles_file = new URL('../styles.css', import.meta.url);

assert.equal(fs.existsSync(tools_hub_page), false, 'tools hub page should be removed after the settings migration');
assert.equal(fs.existsSync(tool_placeholder_page), false, 'tool placeholder page should be removed after the settings migration');

const settings_page_source = fs.readFileSync(settings_page, 'utf8');
assert.match(settings_page_source, /project-reference-shell/, 'settings page should expose the reference project shell');
assert.match(settings_page_source, /global-reference-shell/, 'settings page should expose the global reference shell');

const messages_source = fs.readFileSync(messages_file, 'utf8');
for (const obsolete_key of ['nav.tools', 'nav.toolsHub', 'tools.title', 'tools.commenter.title', 'placeholder.tool.title']) {
  assert.equal(messages_source.includes(`'${obsolete_key}'`), false, `${obsolete_key} should be removed from locale messages`);
}

const styles_source = fs.readFileSync(styles_file, 'utf8');
for (const obsolete_selector of ['.tool-grid', '.tool-card']) {
  assert.equal(styles_source.includes(obsolete_selector), false, `${obsolete_selector} should be removed from shared styles`);
}

console.log('settings workspace cleanup PASSED');
