<script setup lang="ts">
import { Clock3, RotateCcw } from 'lucide-vue-next';

import { commenterStore } from '../../lib/commenterStore';
import { run_status_label } from '../../lib/commenterView';
import { use_messages } from '../../locales/messages';

const { t } = use_messages();
</script>

<template>
  <section class="panel">
    <div class="panel-title">
      <div>
        <h3>{{ t('commenter.history') }}</h3>
        <p>{{ t('commenter.history.help') }}</p>
      </div>
    </div>

    <div
      v-if="commenterStore.state.history_runs.length > 0"
      class="list"
    >
      <div
        v-for="run in commenterStore.state.history_runs"
        :key="run.run_key"
        class="list-item"
      >
        <div>
          <h4>{{ run.run_key }}</h4>
          <p>
            {{ run_status_label(run.status) }} ·
            {{ t('commenter.history.resolved', { done: run.completed_jobs, total: run.total_jobs }) }}
          </p>
        </div>
        <div class="button-row">
          <button
            class="button secondary"
            @click="commenterStore.selectRun(run.run_key)"
          >
            <Clock3 :size="16" />
            {{ t('commenter.focus') }}
          </button>
          <button
            class="button secondary"
            @click="commenterStore.rollbackRun(run.run_key)"
          >
            <RotateCcw :size="16" />
            {{ t('commenter.rollback') }}
          </button>
        </div>
      </div>
    </div>

    <div
      v-else
      class="empty-state"
    >
      {{ t('commenter.history.empty') }}
    </div>
  </section>
</template>
