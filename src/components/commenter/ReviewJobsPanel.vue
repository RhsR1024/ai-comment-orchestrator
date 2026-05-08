<script setup lang="ts">
import { computed, ref } from 'vue';
import { Check, GitCompareArrows, RotateCcw, X } from 'lucide-vue-next';

import { commenterStore } from '../../lib/commenterStore';
import { review_job_message } from '../../lib/commenterView';
import { use_messages } from '../../locales/messages';

const { t } = use_messages();

const review_jobs = computed(
  () =>
    commenterStore.state.selected_run_detail?.jobs.filter((job) => job.status === 'review_needed') ?? []
);
const selected_run_key = computed(() => commenterStore.state.selected_run_detail?.run.run_key ?? null);
const pending_action = ref<string | null>(null);
const action_error = ref<string | null>(null);

async function runReviewAction(
  action_key: string,
  relative_path: string,
  // eslint-disable-next-line no-unused-vars
  work: (...args: [string]) => Promise<unknown>
) {
  const run_key = selected_run_key.value;
  if (!run_key) {
    action_error.value = t('commenter.review.noRun');
    return;
  }

  pending_action.value = `${action_key}:${relative_path}`;
  action_error.value = null;
  try {
    await work(run_key);
  } catch (error) {
    action_error.value = error instanceof Error ? error.message : String(error);
  } finally {
    pending_action.value = null;
  }
}

function isPending(action_key: string, relative_path: string): boolean {
  return pending_action.value === `${action_key}:${relative_path}`;
}
</script>

<template>
  <section class="panel">
    <div class="panel-title">
      <div>
        <h3>{{ t('commenter.review') }}</h3>
        <p>{{ t('commenter.review.help') }}</p>
      </div>
    </div>

    <div
      v-if="review_jobs.length > 0"
      class="list"
    >
      <div
        v-if="action_error"
        class="review-action-error"
        role="alert"
      >
        {{ action_error }}
      </div>
      <div
        v-for="job in review_jobs"
        :key="job.id"
        class="list-item"
      >
        <div>
          <h4>{{ job.relative_path }}</h4>
          <p>{{ review_job_message(job) }}</p>
        </div>
        <div class="button-row">
          <button
            class="button secondary"
            :disabled="isPending('diff', job.relative_path)"
            @click="runReviewAction('diff', job.relative_path, (run_key) => commenterStore.openExternalDiff({ run_key, relative_path: job.relative_path }))"
          >
            <GitCompareArrows :size="16" />
            {{ t('commenter.openDiff') }}
          </button>
          <button
            class="button secondary"
            :disabled="isPending('accept', job.relative_path)"
            @click="runReviewAction('accept', job.relative_path, (run_key) => commenterStore.acceptReviewJob({ run_key, relative_path: job.relative_path }))"
          >
            <Check :size="16" />
            {{ t('commenter.accept') }}
          </button>
          <button
            class="button secondary"
            :disabled="isPending('reject', job.relative_path)"
            @click="runReviewAction('reject', job.relative_path, (run_key) => commenterStore.rejectReviewJob({ run_key, relative_path: job.relative_path }))"
          >
            <X :size="16" />
            {{ t('commenter.reject') }}
          </button>
          <button
            class="button secondary"
            :disabled="isPending('retry', job.relative_path)"
            @click="runReviewAction('retry', job.relative_path, (run_key) => commenterStore.retryJob({ run_key, relative_path: job.relative_path }))"
          >
            <RotateCcw :size="16" />
            {{ t('commenter.retry') }}
          </button>
        </div>
      </div>
    </div>

    <div
      v-else
      class="empty-state"
    >
      {{ t('commenter.review.empty') }}
    </div>
  </section>
</template>

<style scoped>
.review-action-error {
  border: 1px solid rgba(239, 68, 68, 0.24);
  border-radius: 8px;
  background: rgba(239, 68, 68, 0.1);
  color: #fecaca;
  padding: 10px 12px;
}
</style>
