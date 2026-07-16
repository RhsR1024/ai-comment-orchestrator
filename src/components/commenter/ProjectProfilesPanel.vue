<script setup lang="ts">
import { reactive, ref } from 'vue';
import { Folder, FolderOpen, Pencil, Plus, RefreshCcw, Save, Trash2, X } from 'lucide-vue-next';

import { commenterStore } from '../../lib/commenterStore';
import { createDefaultCommenterProjectProfileDraft } from '../../lib/commenterProfileDefaults';
import type { CommenterProjectProfileDraft, CommenterProjectProfileView } from '../../lib/commenterTypes';
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
const is_creating = ref(false);
const editing_project_key = ref<string | null>(null);
const deleting_project_key = ref<string | null>(null);
const save_state = ref<'idle' | 'saving' | 'saved' | 'error'>('idle');
const save_error = ref<string | null>(null);
const success_message = ref<string | null>(null);
const list_error = ref<string | null>(null);

function startCreatingProfile() {
  Object.assign(draft, createDefaultCommenterProjectProfileDraft());
  editing_project_key.value = null;
  save_state.value = 'idle';
  save_error.value = null;
  success_message.value = null;
  list_error.value = null;
  is_creating.value = true;
}

function startEditingProfile(profile: CommenterProjectProfileView) {
  Object.assign(draft, {
    project_key: profile.project_key,
    profile_name: profile.profile_name,
    root_path: profile.root_path,
    include_extensions: [...profile.include_extensions],
    exclude_directories: [...profile.exclude_directories],
    prompt_template: profile.prompt_template,
    settings: { ...profile.settings }
  });
  editing_project_key.value = profile.project_key;
  save_state.value = 'idle';
  save_error.value = null;
  success_message.value = null;
  list_error.value = null;
  is_creating.value = true;
}

function cancelCreatingProfile() {
  is_creating.value = false;
  editing_project_key.value = null;
  save_state.value = 'idle';
  save_error.value = null;
}

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
  return editing_project_key.value ?? draft.profile_name.trim();
}

async function submitProfile() {
  const profile_name = draft.profile_name.trim();
  save_state.value = 'saving';
  save_error.value = null;
  try {
    await commenterStore.saveProfile({
      ...draft,
      project_key: mergedProjectKey(),
      profile_name,
      include_extensions: [...draft.include_extensions],
      exclude_directories: [...draft.exclude_directories],
      settings: {
        ...draft.settings,
        default_max_files: 0
      }
    });
    save_state.value = 'saved';
    success_message.value = t(
      editing_project_key.value ? 'commenter.profile.updateSuccess' : 'commenter.save.success'
    );
    editing_project_key.value = null;
    is_creating.value = false;
  } catch (error) {
    save_state.value = 'error';
    save_error.value = error instanceof Error ? error.message : String(error);
  }
}

async function deleteProfile(profile: CommenterProjectProfileView) {
  if (!window.confirm(t('commenter.profile.deleteConfirm', { name: profile.profile_name }))) {
    return;
  }
  deleting_project_key.value = profile.project_key;
  list_error.value = null;
  success_message.value = null;
  try {
    await commenterStore.deleteProfile(profile.project_key);
    save_state.value = 'saved';
    success_message.value = t('commenter.profile.deleteSuccess');
  } catch (error) {
    save_state.value = 'idle';
    list_error.value = error instanceof Error ? error.message : String(error);
  } finally {
    deleting_project_key.value = null;
  }
}
</script>

<template>
  <section :class="['panel', { 'project-form-panel': props.variant === 'reference' }]">
    <div class="panel-title">
      <div>
        <h3>{{ is_creating ? t(editing_project_key ? 'commenter.profile.editTitle' : 'commenter.profile.addTitle') : t('commenter.profile.listTitle') }}</h3>
        <p>{{ is_creating ? t(editing_project_key ? 'commenter.profile.editHelp' : 'commenter.profile.addHelp') : t('commenter.profile.listHelp') }}</p>
      </div>
      <div class="profile-header-actions">
        <span
          v-if="save_state === 'saved'"
          class="profile-save-feedback profile-save-feedback--ok"
          role="status"
        >
          {{ success_message ?? t('commenter.save.success') }}
        </span>
        <template v-if="!is_creating">
          <button
            class="button ghost profile-refresh-button"
            type="button"
            :aria-label="t('commenter.refresh')"
            :title="t('commenter.refresh')"
            @click="commenterStore.refresh()"
          >
            <RefreshCcw :size="16" />
          </button>
          <button
            class="button"
            type="button"
            @click="startCreatingProfile"
          >
            <Plus :size="16" />
            {{ t('commenter.profile.add') }}
          </button>
        </template>
        <button
          v-else
          class="button ghost"
          type="button"
          @click="cancelCreatingProfile"
        >
          <X :size="16" />
          {{ t('commenter.cancel') }}
        </button>
      </div>
    </div>

    <div class="panel-body">
      <div
        v-if="is_creating"
        class="profile-create-card"
      >
        <div class="field-grid profile-form-grid">
          <div class="field field-span-2">
            <label class="field-label-row">
              <span>{{ t('commenter.profile.profileName') }}</span>
              <span
                class="required-mark"
                aria-hidden="true"
              >*</span>
            </label>
            <input v-model.trim="draft.profile_name">
            <p class="field-hint">
              {{ t('commenter.profile.hint.profileName') }}
            </p>
          </div>
          <div class="field field-span-2">
            <label class="field-label-row">
              <span>{{ t('commenter.profile.rootPath') }}</span>
              <span
                class="required-mark"
                aria-hidden="true"
              >*</span>
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
            <input
              v-model.trim="draft.settings.api_base_url"
              name="api_base_url"
            >
          </div>
          <div class="field">
            <label>API Model</label>
            <input
              v-model.trim="draft.settings.api_model"
              name="api_model"
            >
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
              <span
                class="required-mark"
                aria-hidden="true"
              >*</span>
            </label>
            <textarea v-model="draft.prompt_template" />
            <p class="field-hint">
              {{ t('commenter.profile.hint.promptTemplate') }}
            </p>
          </div>
        </div>

        <label class="checkbox-field">
          <input
            v-model="draft.settings.allow_light_rewrite"
            type="checkbox"
          >
          {{ t('commenter.profile.allowLightRewrite') }}
        </label>

        <div class="button-row">
          <button
            class="button"
            type="button"
            :disabled="save_state === 'saving'"
            @click="submitProfile"
          >
            <Save :size="16" />
            {{ save_state === 'saving' ? t('commenter.save.saving') : t(editing_project_key ? 'commenter.profile.saveChanges' : 'commenter.createProfile') }}
          </button>
          <button
            class="button ghost"
            type="button"
            @click="cancelCreatingProfile"
          >
            {{ t('commenter.cancel') }}
          </button>
          <span
            v-if="save_state === 'error'"
            class="profile-save-feedback profile-save-feedback--error"
            role="alert"
          >
            {{ save_error ?? t('commenter.save.failed') }}
          </span>
        </div>
      </div>

      <div
        v-else-if="commenterStore.state.profiles.length === 0"
        class="profile-empty-state"
      >
        <div
          class="profile-empty-icon"
          aria-hidden="true"
        >
          <Folder :size="22" />
        </div>
        <h4>{{ t('commenter.profile.emptyTitle') }}</h4>
        <p>{{ t('commenter.profile.emptyHelp') }}</p>
        <button
          class="button"
          type="button"
          @click="startCreatingProfile"
        >
          <Plus :size="16" />
          {{ t('commenter.profile.add') }}
        </button>
      </div>

      <div
        v-else
        class="profile-list-section"
      >
        <div class="profile-list-summary">
          <span>{{ t('commenter.profile.count', { count: commenterStore.state.profiles.length }) }}</span>
          <span
            v-if="list_error"
            class="profile-save-feedback profile-save-feedback--error"
            role="alert"
          >
            {{ list_error }}
          </span>
        </div>
        <div class="list profile-list">
          <div
            v-for="profile in commenterStore.state.profiles"
            :key="profile.project_key"
            class="list-item profile-list-item"
          >
            <div class="profile-list-item-main">
              <div
                class="profile-list-item-icon"
                aria-hidden="true"
              >
                <Folder :size="18" />
              </div>
              <div>
                <h4>{{ profile.profile_name }}</h4>
                <p>{{ profile.root_path }}</p>
              </div>
            </div>
            <div class="profile-list-item-actions">
              <span class="status-badge">
                {{ profile.settings.default_run_mode === 'review' ? t('commenter.profile.review') : t('commenter.profile.auto') }}
              </span>
              <button
                class="button ghost profile-icon-action"
                type="button"
                :aria-label="t('commenter.profile.edit')"
                :title="t('commenter.profile.edit')"
                @click="startEditingProfile(profile)"
              >
                <Pencil :size="15" />
              </button>
              <button
                class="button ghost danger profile-icon-action"
                type="button"
                :disabled="deleting_project_key === profile.project_key"
                :aria-label="t('commenter.profile.delete')"
                :title="t('commenter.profile.delete')"
                @click="deleteProfile(profile)"
              >
                <Trash2 :size="15" />
              </button>
            </div>
          </div>
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
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}

.profile-header-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
}

.profile-refresh-button {
  width: 36px;
  min-width: 36px;
  padding-inline: 0;
}

.profile-create-card {
  display: flex;
  flex-direction: column;
  gap: 14px;
  border: 1px solid var(--aco-border);
  border-radius: 10px;
  background: var(--aco-surface-1);
  padding: 18px;
}

.profile-list-section {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.profile-list-summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  color: var(--aco-muted);
  font-size: 12px;
}

.profile-list-item-actions {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 6px;
}

.profile-icon-action {
  width: 30px;
  min-width: 30px;
  height: 30px;
  padding: 0;
}

.profile-list-item {
  min-height: 70px;
  padding: 14px;
}

.profile-list-item-main {
  display: flex;
  min-width: 0;
  align-items: flex-start;
  gap: 10px;
}

.profile-list-item-main > div:last-child {
  min-width: 0;
}

.profile-list-item-main p {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.profile-list-item-icon,
.profile-empty-icon {
  display: grid;
  flex: 0 0 auto;
  place-items: center;
  border: 1px solid rgba(77, 208, 225, 0.2);
  border-radius: 8px;
  background: rgba(77, 208, 225, 0.08);
  color: var(--aco-teal);
}

.profile-list-item-icon {
  width: 34px;
  height: 34px;
}

.profile-empty-state {
  display: flex;
  min-height: 260px;
  align-items: center;
  justify-content: center;
  flex-direction: column;
  border: 1px dashed var(--aco-border);
  border-radius: 10px;
  background: var(--aco-surface-1);
  padding: 32px;
  text-align: center;
}

.profile-empty-icon {
  width: 46px;
  height: 46px;
  margin-bottom: 14px;
}

.profile-empty-state h4 {
  margin: 0;
  color: var(--aco-text);
  font-size: 14px;
}

.profile-empty-state p {
  max-width: 420px;
  margin: 7px 0 18px;
  color: var(--aco-muted);
  font-size: 12px;
  line-height: 1.6;
}

.profile-save-feedback {
  align-self: center;
  font-size: 12px;
}

.profile-save-feedback--ok {
  color: var(--aco-green);
}

.profile-save-feedback--error {
  color: var(--aco-red);
}

.project-form-panel {
  border: 0;
  background: transparent;
  padding: 0;
  border-radius: 0;
}

@media (max-width: 720px) {
  .panel-title {
    align-items: flex-start;
    flex-direction: column;
  }

  .profile-header-actions {
    width: 100%;
    justify-content: flex-start;
    flex-wrap: wrap;
  }

  .profile-list {
    grid-template-columns: 1fr;
  }

  .path-field {
    grid-template-columns: 1fr;
  }
}
</style>
