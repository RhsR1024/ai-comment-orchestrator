import assert from 'node:assert/strict';
import fs from 'node:fs';

const source = fs.readFileSync(new URL('../locales/messages.ts', import.meta.url), 'utf8');
const app_source = fs.readFileSync(new URL('../App.vue', import.meta.url), 'utf8');

assert.doesNotMatch(source, /'en-US'/, "en-US locale must be removed");
assert.doesNotMatch(source, /set_locale/, "locale switching must be removed");
assert.doesNotMatch(source, /localStorage/, "locale storage must be removed");
assert.match(source, /export type LocaleCode = 'zh-CN'/, 'LocaleCode should be Chinese-only');
assert.doesNotMatch(app_source, /locale_options/, 'App.vue should not render a locale switch');
assert.doesNotMatch(app_source, /class="app-header"/, 'App.vue should drop the legacy app header');

console.log('commenter locale PASSED');
