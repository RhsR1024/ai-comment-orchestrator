<script setup lang="ts">
import { computed, reactive, watch } from 'vue';
import { Pause, Play, RotateCcw, Square, TimerReset, Trash2 } from 'lucide-vue-next';

import { commenterStore } from '../../lib/commenterStore';
import { run_progress_percent, run_status_label } from '../../lib/commenterView';
import type { CommenterEnqueueRunRequest, CommenterRunRecord } from '../../lib/commenterTypes';
import { use_messages } from '../../locales/messages';

const props = withDefaults(
  defineProps<{ variant?: 'panel' | 'rail' }>(),
  {
    variant: 'panel'
  }
);

const { t } = use_messages();

const form = reactive<CommenterEnqueueRunRequest>({
  profile_key: '',
  requested_by: 'browser-demo',
  run_mode: 'review',
  max_workers: 2,
  max_retries: 1,
  max_files: 2,
  allow_light_rewrite: true,
  json_handling_strategy: 'sidecar_only'
});

const can_enqueue = computed(() => commenterStore.state.profiles.length > 0 && form.profile_key.length > 0);

const profiles = computed(() => commenterStore.state.profiles);
const runs = computed(() => commenterStore.state.runs);
const has_runs = computed(() => runs.value.length > 0);

function applyProfileDefaults(profile_key: string) {
  const profile = profiles.value.find((entry) => entry.project_key === profile_key);
  if (!profile) {
    return;
  }
  form.run_mode = profile.settings.default_run_mode;
  form.max_workers = profile.settings.default_max_workers;
  form.max_retries = profile.settings.default_max_retries;
  form.max_files = profile.settings.default_max_files;
  form.allow_light_rewrite = profile.settings.allow_light_rewrite;
  form.json_handling_strategy = profile.settings.json_handling_strategy;
}

watch(
  profiles,
  (next_profiles) => {
    if (next_profiles.length === 0) {
      form.profile_key = '';
      return;
    }

    const has_selected_profile = next_profiles.some((profile) => profile.project_key === form.profile_key);
    if (!has_selected_profile) {
      form.profile_key = next_profiles[0].project_key;
    }
    applyProfileDefaults(form.profile_key);
  },
  { immediate: true }
);

watch(
  () => form.profile_key,
  (profile_key) => {
    if (profile_key) {
      applyProfileDefaults(profile_key);
    }
  }
);

function buildRunRequest(): CommenterEnqueueRunRequest {
  return {
    profile_key: form.profile_key,
    requested_by: form.requested_by,
    run_mode: form.run_mode,
    max_workers: form.max_workers,
    max_retries: form.max_retries,
    max_files: form.max_files,
    allow_light_rewrite: form.allow_light_rewrite,
    json_handling_strategy: form.json_handling_strategy
  };
}

async function enqueueRun() {
  await commenterStore.enqueueRun(buildRunRequest());
}

async function enqueueAndStartRun() {
  const run = await commenterStore.enqueueRun(buildRunRequest());
  await commenterStore.startRun(run.run_key);
}

function canDeleteRun(run: CommenterRunRecord): boolean {
  return run.status !== 'running' && run.status !== 'pausing';
}
</script>

<template>
  <section :class="['panel', { 'queue-rail-panel': props.variant === 'rail' }]">
    <div
      v-if="props.variant !== 'rail'"
      class="panel-title"
    >
      <div>
        <h3>{{ t('commenter.queue') }}</h3>
        <p>{{ t('commenter.queue.help') }}</p>
      </div>
    </div>

    <div class="panel-body">
      <div
        v-if="props.variant === 'rail'"
        class="queue-rail-form"
      >
        <div class="queue-rail-form-title">
          <span>{{ t('commenter.queue.profile') }}</span>
          <span class="status-badge">{{ profiles.length }}</span>
        </div>

        <div
          v-if="profiles.length > 0"
          class="queue-rail-controls"
        >
          <label class="queue-rail-field">
            <span>{{ t('commenter.queue.profile') }}</span>
            <select
              v-model="form.profile_key"
              class="queue-rail-control queue-rail-control--select"
            >
              <option
                disabled
                value=""
              >
                {{ t('commenter.queue.chooseProfile') }}
              </option>
              <option
                v-for="profile in profiles"
                :key="profile.project_key"
                :value="profile.project_key"
              >
                {{ profile.profile_name }}
              </option>
            </select>
          </label>

          <div class="queue-rail-settings">
            <label class="queue-rail-field">
              <span>{{ t('commenter.queue.runMode') }}</span>
              <select
                v-model="form.run_mode"
                class="queue-rail-control queue-rail-control--select"
              >
                <option value="review">
                  {{ t('commenter.profile.review') }}
                </option>
                <option value="auto">
                  {{ t('commenter.profile.auto') }}
                </option>
              </select>
            </label>
            <label class="queue-rail-field">
              <span>{{ t('commenter.queue.maxFiles') }}</span>
              <input
                v-model.number="form.max_files"
                class="queue-rail-control queue-rail-control--number"
                type="number"
                min="1"
                inputmode="numeric"
              >
            </label>
          </div>

          <button
            class="button queue-rail-run-button"
            :disabled="!can_enqueue"
            @click="enqueueAndStartRun"
          >
            <Play :size="16" />
            {{ t('commenter.queue.enqueueAndStart') }}
          </button>
        </div>

        <div
          v-else
          class="empty-state queue-rail-empty"
        >
          <span>{{ t('commenter.queue.emptyProfiles') }}</span>
          <RouterLink
            class="button secondary queue-rail-settings-link"
            to="/settings"
          >
            {{ t('nav.projectConfig') }}
          </RouterLink>
        </div>
      </div>

      <div
        v-if="props.variant !== 'rail'"
        class="field-grid queue-form-grid"
      >
        <div class="field field-span-2">
          <label>{{ t('commenter.queue.profile') }}</label>
          <select v-model="form.profile_key">
            <option
              disabled
              value=""
            >
              {{ t('commenter.queue.chooseProfile') }}
            </option>
            <option
              v-for="profile in profiles"
              :key="profile.project_key"
              :value="profile.project_key"
            >
              {{ profile.profile_name }}
            </option>
          </select>
        </div>
        <div class="field">
          <label>{{ t('commenter.queue.runMode') }}</label>
          <select v-model="form.run_mode">
            <option value="review">
              {{ t('commenter.profile.review') }}
            </option>
            <option value="auto">
              {{ t('commenter.profile.auto') }}
            </option>
          </select>
        </div>
        <div class="field">
          <label>{{ t('commenter.queue.maxWorkers') }}</label>
          <input
            v-model.number="form.max_workers"
            type="number"
            min="1"
            inputmode="numeric"
          >
        </div>
        <div class="field">
          <label>{{ t('commenter.queue.retries') }}</label>
          <input
            v-model.number="form.max_retries"
            type="number"
            min="0"
            inputmode="numeric"
          >
        </div>
        <div class="field field-span-2">
          <label>{{ t('commenter.queue.maxFiles') }}</label>
          <input
            v-model.number="form.max_files"
            type="number"
            min="1"
            inputmode="numeric"
          >
        </div>
      </div>

      <div
        v-if="props.variant !== 'rail'"
        class="button-row"
      >
        <button
          class="button"
          :disabled="!can_enqueue"
          @click="enqueueRun"
        >
          <Play :size="16" />
          {{ t('commenter.enqueueRun') }}
        </button>
      </div>

      <div
        v-if="!has_runs && profiles.length > 0"
        class="empty-state queue-empty-state"
      >
        {{ t('commenter.queue.emptyRuns') }}
      </div>

      <div
        v-else
        class="table-shell"
      >
        <table class="data-table">
          <thead>
            <tr>
              <th>{{ t('commenter.queue.column.run') }}</th>
              <th>{{ t('commenter.queue.column.status') }}</th>
              <th>{{ t('commenter.queue.column.progress') }}</th>
              <th>{{ t('commenter.queue.column.actions') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="run in runs"
              :key="run.run_key"
            >
              <td>
                <button
                  class="button ghost"
                  @click="commenterStore.selectRun(run.run_key)"
                >
                  {{ run.run_key }}
                </button>
                <div class="muted">
                  {{ run.profile_key }}
                </div>
              </td>
              <td>
                <span class="status-badge">{{ run_status_label(run.status) }}</span>
              </td>
              <td>
                <div class="progress">
                  <span :style="{ width: `${run_progress_percent(run)}%` }" />
                </div>
                <div class="muted">
                  {{ run_progress_percent(run) }}%
                </div>
              </td>
              <td>
                <div class="button-row">
                  <button
                    class="button secondary"
                    :title="t('commenter.start')"
                    :aria-label="t('commenter.start')"
                    @click="commenterStore.startRun(run.run_key)"
                  >
                    <Play :size="16" />
                  </button>
                  <button
                    class="button secondary"
                    :title="t('commenter.pause')"
                    :aria-label="t('commenter.pause')"
                    @click="commenterStore.pauseRun(run.run_key)"
                  >
                    <Pause :size="16" />
                  </button>
                  <button
                    class="button secondary"
                    :title="t('commenter.resume')"
                    :aria-label="t('commenter.resume')"
                    @click="commenterStore.resumeRun(run.run_key)"
                  >
                    <RotateCcw :size="16" />
                  </button>
                  <button
                    class="button secondary"
                    :title="t('commenter.cancel')"
                    :aria-label="t('commenter.cancel')"
                    @click="commenterStore.cancelRun(run.run_key)"
                  >
                    <Square :size="16" />
                  </button>
                  <button
                    class="button secondary"
                    :title="t('commenter.rollback')"
                    :aria-label="t('commenter.rollback')"
                    @click="commenterStore.rollbackRun(run.run_key)"
                  >
                    <TimerReset :size="16" />
                  </button>
                  <button
                    class="button secondary danger"
                    :disabled="!canDeleteRun(run)"
                    :title="t('commenter.delete')"
                    :aria-label="t('commenter.delete')"
                    @click="commenterStore.deleteRun(run.run_key)"
                  >
                    <Trash2 :size="16" />
                  </button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <div
        v-if="has_runs"
        class="queue-run-cards"
      >
        <article
          v-for="run in runs"
          :key="`${run.run_key}-card`"
          class="queue-run-card"
        >
          <div class="queue-card-head">
            <div>
              <button
                class="button ghost queue-card-title"
                @click="commenterStore.selectRun(run.run_key)"
              >
                {{ run.run_key }}
              </button>
              <div class="muted">
                {{ run.profile_key }}
              </div>
            </div>
            <span class="status-badge">{{ run_status_label(run.status) }}</span>
          </div>

          <div class="queue-card-grid">
            <div>
              <span class="queue-card-label">{{ t('commenter.queue.column.progress') }}</span>
              <div class="progress">
                <span :style="{ width: `${run_progress_percent(run)}%` }" />
              </div>
              <div class="muted">
                {{ run_progress_percent(run) }}%
              </div>
            </div>
          </div>

          <div class="queue-card-actions">
            <span class="queue-card-label">{{ t('commenter.queue.mobileActions') }}</span>
            <div class="button-row">
              <button
                class="button secondary"
                :title="t('commenter.start')"
                :aria-label="t('commenter.start')"
                @click="commenterStore.startRun(run.run_key)"
              >
                <Play :size="16" />
              </button>
              <button
                class="button secondary"
                :title="t('commenter.pause')"
                :aria-label="t('commenter.pause')"
                @click="commenterStore.pauseRun(run.run_key)"
              >
                <Pause :size="16" />
              </button>
              <button
                class="button secondary"
                :title="t('commenter.resume')"
                :aria-label="t('commenter.resume')"
                @click="commenterStore.resumeRun(run.run_key)"
              >
                <RotateCcw :size="16" />
              </button>
              <button
                class="button secondary"
                :title="t('commenter.cancel')"
                :aria-label="t('commenter.cancel')"
                @click="commenterStore.cancelRun(run.run_key)"
              >
                <Square :size="16" />
              </button>
              <button
                class="button secondary"
                :title="t('commenter.rollback')"
                :aria-label="t('commenter.rollback')"
                @click="commenterStore.rollbackRun(run.run_key)"
              >
                <TimerReset :size="16" />
              </button>
              <button
                class="button secondary danger"
                :disabled="!canDeleteRun(run)"
                :title="t('commenter.delete')"
                :aria-label="t('commenter.delete')"
                @click="commenterStore.deleteRun(run.run_key)"
              >
                <Trash2 :size="16" />
              </button>
            </div>
          </div>
        </article>
      </div>
    </div>
  </section>
</template>

<style scoped>
.queue-rail-panel {
  border: 0;
  border-radius: 0;
  background: transparent;
  padding: 0;
}

.queue-rail-panel .panel-body {
  gap: 0;
}

.queue-rail-form {
  display: flex;
  flex-direction: column;
  gap: 12px;
  border-bottom: 1px solid var(--aco-border);
  background: rgba(255, 255, 255, 0.015);
  padding: 14px 12px;
}

.queue-rail-form-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  color: var(--aco-text);
  font-size: 12px;
  font-weight: 600;
}

.queue-rail-controls {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.queue-rail-settings {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(86px, 0.42fr);
  gap: 8px;
}

.queue-rail-run-button {
  justify-content: center;
  width: 100%;
}

.queue-rail-empty {
  display: grid;
  gap: 10px;
  padding: 10px;
}

.queue-rail-settings-link {
  justify-content: center;
  text-decoration: none;
}

.queue-empty-state {
  margin: 12px;
}

.queue-rail-panel .table-shell {
  padding: 0 12px 12px;
}

.queue-run-cards {
  display: none;
  flex-direction: column;
  gap: 12px;
}

.queue-run-card {
  display: flex;
  flex-direction: column;
  gap: 14px;
  border: 1px solid rgba(146, 177, 174, 0.12);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.02);
  padding: 14px;
}

.queue-card-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.queue-card-title {
  padding-left: 0;
}

.queue-card-grid {
  display: grid;
  grid-template-columns: 1fr;
  gap: 10px;
}

.queue-card-label {
  display: block;
  margin-bottom: 6px;
  color: #8fb0ac;
  font-size: 12px;
  text-transform: uppercase;
}

.queue-card-actions {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.button.danger {
  border-color: rgba(231, 111, 111, 0.24);
  color: #ffc4c4;
}

@media (max-width: 720px) {
  .table-shell {
    display: none;
  }

  .queue-run-cards {
    display: flex;
  }
}
</style>
