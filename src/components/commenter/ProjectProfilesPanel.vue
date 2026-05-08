<script setup lang="ts">
import { reactive } from 'vue';
import { FolderOpen, FolderPlus, RefreshCcw, Save } from 'lucide-vue-next';

import { commenterStore } from '../../lib/commenterStore';
import { createDefaultCommenterProjectProfileDraft } from '../../lib/commenterProfileDefaults';
import type { CommenterProjectProfileDraft } from '../../lib/commenterTypes';
import { pickProjectRootPath } from '../../lib/tauri';
import { use_messages } from '../../locales/messages';

const { t } = use_messages();

const props = withDefaults(
  defineProps<{ variant?: 'panel' | 'reference' }>(),
  {
    variant: 'panel'
  }
);

const draft = reactive<CommenterProjectProfileDraft>(createDefaultCommenterProjectProfileDraft());

function splitCommaSeparatedValues(raw: string): string[] {
  return raw
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);
}

async function chooseRootPath() {
  const selectedPath = await pickProjectRootPath(draft.root_path);
  if (selectedPath) {
    draft.root_path = selectedPath;
  }
}

function mergedProjectKey(): string {
  return draft.profile_name.trim();
}

async function submitProfile() {
  const profile_name = draft.profile_name.trim();
  await commenterStore.saveProfile({
    ...draft,
    project_key: mergedProjectKey(),
    profile_name,
    include_extensions: [...draft.include_extensions],
    exclude_directories: [...draft.exclude_directories]
  });
}
</script>

<template>
  <section :class="['panel', { 'project-form-panel': props.variant === 'reference' }]">
    <div class="panel-title">
      <div>
        <h3>{{ t('commenter.profiles') }}</h3>
        <p>{{ t('commenter.profiles.help') }}</p>
      </div>
      <button
        class="button ghost"
        @click="commenterStore.refresh()"
      >
        <RefreshCcw :size="16" />
      </button>
    </div>

    <div class="panel-body">
      <div class="field-grid profile-form-grid">
        <div class="field field-span-2">
          <label class="field-label-row">
            <span>{{ t('commenter.profile.profileName') }}</span>
            <span class="required-mark" aria-hidden="true">*</span>
          </label>
          <input v-model.trim="draft.profile_name">
          <p class="field-hint">
            {{ t('commenter.profile.hint.profileName') }}
          </p>
        </div>
        <div class="field field-span-2">
          <label class="field-label-row">
            <span>{{ t('commenter.profile.rootPath') }}</span>
            <span class="required-mark" aria-hidden="true">*</span>
          </label>
          <div class="path-field">
            <input v-model.trim="draft.root_path">
            <button
              class="button secondary path-button"
              type="button"
              @click="chooseRootPath"
            >
              <FolderOpen :size="16" />
              {{ t('commenter.profile.browseRootPath') }}
            </button>
          </div>
          <p class="field-hint">
            {{ t('commenter.profile.hint.rootPath') }}
          </p>
        </div>
        <div class="field field-span-2">
          <label>{{ t('commenter.profile.includeExtensions') }}</label>
          <input
            :value="draft.include_extensions.join(', ')"
            @input="draft.include_extensions = splitCommaSeparatedValues(String(($event.target as HTMLInputElement).value))"
          >
          <p class="field-hint">
            {{ t('commenter.profile.hint.includeExtensions') }}
          </p>
        </div>
        <div class="field field-span-2">
          <label>{{ t('commenter.profile.excludeDirectories') }}</label>
          <input
            :value="draft.exclude_directories.join(', ')"
            @input="draft.exclude_directories = splitCommaSeparatedValues(String(($event.target as HTMLInputElement).value))"
          >
          <p class="field-hint">
            {{ t('commenter.profile.hint.excludeDirectories') }}
          </p>
        </div>
        <div class="field">
          <label>API Base URL</label>
          <input v-model.trim="draft.settings.api_base_url" name="api_base_url">
        </div>
        <div class="field">
          <label>API Model</label>
          <input v-model.trim="draft.settings.api_model" name="api_model">
        </div>
        <div class="field">
          <label>{{ t('commenter.profile.defaultMode') }}</label>
          <select v-model="draft.settings.default_run_mode">
            <option value="review">
              {{ t('commenter.profile.review') }}
            </option>
            <option value="auto">
              {{ t('commenter.profile.auto') }}
            </option>
          </select>
        </div>
        <div class="field">
          <label>{{ t('commenter.profile.maxFiles') }}</label>
          <input
            v-model.number="draft.settings.default_max_files"
            type="number"
            min="1"
            inputmode="numeric"
          >
        </div>
        <div class="field">
          <label>Request Timeout (s)</label>
          <input
            v-model.number="draft.settings.request_timeout_secs"
            type="number"
            min="1"
            name="request_timeout_secs"
          >
        </div>
        <div class="field field-span-2">
          <label class="field-label-row">
            <span>{{ t('commenter.profile.promptTemplate') }}</span>
            <span class="required-mark" aria-hidden="true">*</span>
          </label>
          <textarea v-model="draft.prompt_template" />
          <p class="field-hint">
            {{ t('commenter.profile.hint.promptTemplate') }}
          </p>
        </div>
      </div>

      <label class="checkbox-field">
        <input v-model="draft.settings.allow_light_rewrite" type="checkbox">
        {{ t('commenter.profile.allowLightRewrite') }}
      </label>

      <div class="button-row">
        <button class="button" @click="submitProfile">
          <Save :size="16" />
          {{ t('commenter.createProfile') }}
        </button>
      </div>

      <div class="list profile-list">
        <div
          v-for="profile in commenterStore.state.profiles"
          :key="profile.project_key"
          class="list-item"
        >
          <div>
            <h4>{{ profile.profile_name }}</h4>
            <p>{{ profile.root_path }}</p>
          </div>
          <span class="status-badge">
            <FolderPlus :size="14" />
            {{ profile.settings.default_run_mode }}
          </span>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.field-label-row {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.path-field {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 10px;
}

.path-button {
  justify-content: center;
  min-width: 112px;
  white-space: nowrap;
}

.profile-list {
  padding-top: 8px;
  border-top: 1px solid var(--aco-border);
}

.project-form-panel {
  border: 0;
  background: transparent;
  padding: 0;
  border-radius: 0;
}

@media (max-width: 720px) {
  .path-field {
    grid-template-columns: 1fr;
  }
}
</style>
