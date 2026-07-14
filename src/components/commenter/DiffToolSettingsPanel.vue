<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import { Eye, EyeOff, FolderOpen, Save } from 'lucide-vue-next';

import { commenterStore } from '../../lib/commenterStore';
import { commenterApi } from '../../lib/tauri';
import { use_messages } from '../../locales/messages';

const { t } = use_messages();

type GlobalSettingsSection =
  | 'api-credentials'
  | 'concurrency-quota'
  | 'diff-tool'
  | 'storage-logs'
  | 'about-settings';

const props = withDefaults(
  defineProps<{
    variant?: 'panel' | 'reference';
    activeSection?: GlobalSettingsSection;
  }>(),
  {
    variant: 'panel',
    activeSection: 'api-credentials'
  }
);

const form = reactive({
  command_template: 'code --diff "{before}" "{after}"',
  global_max_workers: 2,
  api_concurrency_limit: 2,
  api_bearer_token: ''
});

const show_token = ref(false);
const save_state = ref<'idle' | 'saving' | 'saved' | 'error'>('idle');
const save_error = ref<string | null>(null);
let saved_indicator_timer: number | null = null;

let diff_seeded = false;
let app_seeded = false;

watch(
  () => commenterStore.state.diff_tool_settings,
  (value) => {
    if (!value) {
      return;
    }
    if (!diff_seeded) {
      form.command_template = value.command_template;
      diff_seeded = true;
    }
  },
  { immediate: true }
);

watch(
  () => commenterStore.state.app_settings,
  (value) => {
    if (!value) {
      return;
    }
    if (!app_seeded) {
      form.global_max_workers = value.global_max_workers;
      form.api_concurrency_limit = value.api_concurrency_limit;
      form.api_bearer_token = value.api_bearer_token;
      app_seeded = true;
    }
  },
  { immediate: true }
);

const data_paths = computed(() => commenterStore.state.data_paths);
const app_version = computed(
  () => (import.meta.env.VITE_APP_VERSION as string | undefined) ?? 'dev'
);

function flashSaved() {
  save_state.value = 'saved';
  if (saved_indicator_timer !== null) {
    window.clearTimeout(saved_indicator_timer);
  }
  saved_indicator_timer = window.setTimeout(() => {
    save_state.value = 'idle';
    saved_indicator_timer = null;
  }, 2400);
}

async function saveSettings() {
  save_state.value = 'saving';
  save_error.value = null;
  try {
    await commenterStore.saveDiffToolSettings({
      command_template: form.command_template
    });
    await commenterStore.saveAppSettings({
      global_max_workers: form.global_max_workers,
      api_concurrency_limit: form.api_concurrency_limit,
      api_bearer_token: form.api_bearer_token
    });
    flashSaved();
  } catch (error) {
    save_state.value = 'error';
    save_error.value = error instanceof Error ? error.message : String(error);
    throw error;
  }
}

function resetSettings() {
  form.command_template = 'code --diff "{before}" "{after}"';
  form.global_max_workers = 2;
  form.api_concurrency_limit = 2;
  form.api_bearer_token = '';
  save_state.value = 'idle';
  save_error.value = null;
}

async function openPath(path: string | undefined | null) {
  if (!path) return;
  try {
    await commenterApi.openPath(path);
  } catch (error) {
    save_state.value = 'error';
    save_error.value = error instanceof Error ? error.message : String(error);
  }
}

defineExpose({ saveSettings, resetSettings });
</script>

<template>
  <section :class="['panel', { 'global-form-panel': props.variant === 'reference' }]">
    <section
      v-show="props.variant !== 'reference' || props.activeSection === 'api-credentials'"
      id="api-credentials"
      class="global-section"
      role="tabpanel"
      aria-labelledby="global-tab-api-credentials"
    >
      <header class="global-section-header">
        <h3>{{ t('global.section.apiCredentials') }}</h3>
        <p>{{ t('global.section.apiCredentialsHelp') }}</p>
      </header>
      <div class="field field-span-2">
        <label>{{ t('commenter.diff.apiBearerToken') }}</label>
        <div class="token-input">
          <input
            v-model.trim="form.api_bearer_token"
            :type="show_token ? 'text' : 'password'"
            autocomplete="off"
            spellcheck="false"
            name="api_bearer_token"
          >
          <button
            type="button"
            class="token-toggle"
            :aria-label="show_token ? t('commenter.token.hide') : t('commenter.token.show')"
            @click="show_token = !show_token"
          >
            <component :is="show_token ? EyeOff : Eye" :size="14" />
          </button>
        </div>
        <p class="field-hint">{{ t('commenter.diff.hint.apiBearerToken') }}</p>
      </div>
      <div class="credentials-status-pill" data-state="unverified">
        {{ t('global.credential.notVerified') }}
      </div>
    </section>

    <section
      v-show="props.variant !== 'reference' || props.activeSection === 'concurrency-quota'"
      id="concurrency-quota"
      class="global-section"
      role="tabpanel"
      aria-labelledby="global-tab-concurrency-quota"
    >
      <header class="global-section-header">
        <h3>{{ t('global.section.concurrencyQuota') }}</h3>
        <p>{{ t('global.section.concurrencyQuotaHelp') }}</p>
      </header>
      <div class="field-grid">
        <div class="field">
          <label>{{ t('commenter.diff.globalMaxWorkers') }}</label>
          <input v-model.number="form.global_max_workers" type="number" min="1">
        </div>
        <div class="field">
          <label>{{ t('commenter.diff.apiConcurrencyLimit') }}</label>
          <input v-model.number="form.api_concurrency_limit" type="number" min="1">
        </div>
      </div>
    </section>

    <section
      v-show="props.variant !== 'reference' || props.activeSection === 'diff-tool'"
      id="diff-tool"
      class="global-section"
      role="tabpanel"
      aria-labelledby="global-tab-diff-tool"
    >
      <header class="global-section-header">
        <h3>{{ t('global.section.diffTool') }}</h3>
        <p>{{ t('global.section.diffToolHelp') }}</p>
      </header>
      <div class="field">
        <label>{{ t('commenter.diff.commandTemplate') }}</label>
        <input v-model="form.command_template" class="mono">
        <p class="field-hint">{{ t('global.diff.placeholdersHelp') }}</p>
      </div>
    </section>

    <section
      v-show="props.variant !== 'reference' || props.activeSection === 'storage-logs'"
      id="storage-logs"
      class="global-section"
      role="tabpanel"
      aria-labelledby="global-tab-storage-logs"
    >
      <header class="global-section-header">
        <h3>{{ t('global.section.storageLogs') }}</h3>
        <p>{{ t('global.section.storageLogsHelp') }}</p>
      </header>
      <dl class="global-readonly-list storage-paths">
        <div>
          <dt>{{ t('global.storage.dataRoot') }}</dt>
          <dd class="storage-path-row">
            <span class="mono storage-path-text">{{ data_paths?.data_root || '—' }}</span>
            <button
              type="button"
              class="button ghost storage-open-button"
              :disabled="!data_paths?.data_root"
              @click="openPath(data_paths?.data_root)"
            >
              <FolderOpen :size="13" />
              {{ t('commenter.openFolder') }}
            </button>
          </dd>
        </div>
        <div>
          <dt>{{ t('global.storage.artifactsRoot') }}</dt>
          <dd class="storage-path-row">
            <span class="mono storage-path-text">{{ data_paths?.artifacts_root || '—' }}</span>
            <button
              type="button"
              class="button ghost storage-open-button"
              :disabled="!data_paths?.artifacts_root"
              @click="openPath(data_paths?.artifacts_root)"
            >
              <FolderOpen :size="13" />
              {{ t('commenter.openFolder') }}
            </button>
          </dd>
        </div>
        <div>
          <dt>{{ t('global.storage.databaseFile') }}</dt>
          <dd class="storage-path-row">
            <span class="mono storage-path-text">{{ data_paths?.database_path || '—' }}</span>
            <button
              type="button"
              class="button ghost storage-open-button"
              :disabled="!data_paths?.database_path"
              @click="openPath(data_paths?.database_path)"
            >
              <FolderOpen :size="13" />
              {{ t('commenter.openFolder') }}
            </button>
          </dd>
        </div>
      </dl>
    </section>

    <section
      v-show="props.variant !== 'reference' || props.activeSection === 'about-settings'"
      id="about-settings"
      class="global-section"
      role="tabpanel"
      aria-labelledby="global-tab-about-settings"
    >
      <header class="global-section-header">
        <h3>{{ t('global.section.about') }}</h3>
      </header>
      <dl class="global-readonly-list">
        <div>
          <dt>{{ t('global.about.version') }}</dt>
          <dd class="mono">{{ app_version }}</dd>
        </div>
        <div>
          <dt>{{ t('global.about.repository') }}</dt>
          <dd class="mono">ai-comment-orchestrator</dd>
        </div>
      </dl>
    </section>

    <div v-if="props.variant !== 'reference'" class="button-row">
      <button class="button" :disabled="save_state === 'saving'" @click="saveSettings">
        <Save :size="16" />
        {{ t('commenter.save') }}
      </button>
      <span v-if="save_state === 'saved'" class="save-indicator save-indicator--ok">
        {{ t('commenter.save.success') }}
      </span>
      <span v-else-if="save_state === 'error'" class="save-indicator save-indicator--error">
        {{ save_error ?? t('commenter.save.failed') }}
      </span>
    </div>
    <div v-else class="save-feedback-row">
      <span v-if="save_state === 'saved'" class="save-indicator save-indicator--ok">
        {{ t('commenter.save.success') }}
      </span>
      <span v-else-if="save_state === 'error'" class="save-indicator save-indicator--error">
        {{ save_error ?? t('commenter.save.failed') }}
      </span>
    </div>
  </section>
</template>

<style scoped>
.token-input {
  position: relative;
  display: flex;
  align-items: center;
}

.token-input input {
  flex: 1;
  padding-right: 36px;
}

.token-toggle {
  position: absolute;
  right: 6px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--aco-muted);
  cursor: pointer;
}

.token-toggle:hover {
  color: var(--aco-text);
  background: var(--aco-surface-3);
}

.storage-paths > div {
  align-items: center;
}

.storage-path-row {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.storage-path-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.storage-open-button {
  flex: 0 0 auto;
  padding: 4px 10px;
  font-size: 12px;
}

.save-feedback-row {
  padding: 0 0 12px;
}

.save-indicator {
  font-size: 12px;
}

.save-indicator--ok {
  color: var(--aco-green);
}

.save-indicator--error {
  color: var(--aco-red);
}
</style>
