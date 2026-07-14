<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { Box, ClipboardCheck, Globe2 } from 'lucide-vue-next';

import DiffToolSettingsPanel from '../components/commenter/DiffToolSettingsPanel.vue';
import ProjectProfilesPanel from '../components/commenter/ProjectProfilesPanel.vue';
import QueueRunsTable from '../components/commenter/QueueRunsTable.vue';
import ReviewJobsPanel from '../components/commenter/ReviewJobsPanel.vue';
import RunHeaderStrip from '../components/commenter/RunHeaderStrip.vue';
import RunDetailPanel from '../components/commenter/RunDetailPanel.vue';
import { commenterStore } from '../lib/commenterStore';
import { use_messages } from '../locales/messages';

const { t } = use_messages();

type WorkspaceMode = 'project' | 'run' | 'review' | 'global';
type GlobalSettingsSection =
  | 'api-credentials'
  | 'concurrency-quota'
  | 'diff-tool'
  | 'storage-logs'
  | 'about-settings';
const props = withDefaults(
  defineProps<{ workspaceMode?: WorkspaceMode }>(),
  {
    workspaceMode: 'project'
  }
);

const diffSettingsPanel = ref<{
  saveSettings: () => Promise<void>;
  resetSettings: () => void;
} | null>(null);
const activeGlobalSection = ref<GlobalSettingsSection>('api-credentials');

async function saveGlobalSettings() {
  await diffSettingsPanel.value?.saveSettings();
}

function resetGlobalSettings() {
  diffSettingsPanel.value?.resetSettings();
}

onMounted(() => {
  void commenterStore.initialize();
});
</script>

<template>
  <section v-if="props.workspaceMode === 'project'" class="project-reference-shell">
    <header class="project-reference-header">
      <div class="project-title-row">
        <Box class="project-title-icon" :size="15" aria-hidden="true" />
        <div>
          <h1>{{ t('project.title') }}</h1>
          <p>{{ t('project.help') }}</p>
        </div>
      </div>
    </header>
    <div class="project-reference-content">
      <div
        v-if="commenterStore.state.error_message"
        class="empty-state"
        role="alert"
      >
        {{ commenterStore.state.error_message }}
      </div>
      <ProjectProfilesPanel variant="reference" />
    </div>
  </section>

  <section v-else-if="props.workspaceMode === 'review'" class="review-reference-shell">
    <header class="review-reference-header">
      <div class="review-title-row">
        <ClipboardCheck class="review-title-icon" :size="15" aria-hidden="true" />
        <div>
          <h1>{{ t('commenter.review') }}</h1>
          <p>{{ t('commenter.review.manualHelp') }}</p>
        </div>
      </div>
    </header>
    <div class="review-reference-grid">
      <QueueRunsTable />
      <ReviewJobsPanel />
    </div>
  </section>

  <section v-else-if="props.workspaceMode === 'global'" class="global-reference-shell">
    <header class="global-reference-header">
      <div class="global-title-row">
        <Globe2 class="global-title-icon" :size="15" aria-hidden="true" />
        <div>
          <h1>{{ t('global.title') }}</h1>
          <p>{{ t('global.help') }}</p>
        </div>
      </div>
      <div class="global-top-actions">
        <button class="button ghost" type="button" @click="resetGlobalSettings">
          {{ t('global.resetDefaults') }}
        </button>
        <button class="button" type="button" @click="saveGlobalSettings">
          {{ t('commenter.save') }}
        </button>
      </div>
    </header>

    <div class="global-reference-grid">
      <nav
        class="global-subnav"
        aria-label="全局设置分区"
        role="tablist"
      >
        <button
          id="global-tab-api-credentials"
          type="button"
          role="tab"
          :aria-selected="activeGlobalSection === 'api-credentials'"
          :class="{ active: activeGlobalSection === 'api-credentials' }"
          @click="activeGlobalSection = 'api-credentials'"
        >
          {{ t('global.section.apiCredentials') }}
        </button>
        <button
          id="global-tab-concurrency-quota"
          type="button"
          role="tab"
          :aria-selected="activeGlobalSection === 'concurrency-quota'"
          :class="{ active: activeGlobalSection === 'concurrency-quota' }"
          @click="activeGlobalSection = 'concurrency-quota'"
        >
          {{ t('global.section.concurrencyQuota') }}
        </button>
        <button
          id="global-tab-diff-tool"
          type="button"
          role="tab"
          :aria-selected="activeGlobalSection === 'diff-tool'"
          :class="{ active: activeGlobalSection === 'diff-tool' }"
          @click="activeGlobalSection = 'diff-tool'"
        >
          {{ t('global.section.diffTool') }}
        </button>
        <button
          id="global-tab-storage-logs"
          type="button"
          role="tab"
          :aria-selected="activeGlobalSection === 'storage-logs'"
          :class="{ active: activeGlobalSection === 'storage-logs' }"
          @click="activeGlobalSection = 'storage-logs'"
        >
          {{ t('global.section.storageLogs') }}
        </button>
        <button
          id="global-tab-about-settings"
          type="button"
          role="tab"
          :aria-selected="activeGlobalSection === 'about-settings'"
          :class="{ active: activeGlobalSection === 'about-settings' }"
          @click="activeGlobalSection = 'about-settings'"
        >
          {{ t('global.section.about') }}
        </button>
      </nav>
      <div class="global-reference-content">
        <DiffToolSettingsPanel
          ref="diffSettingsPanel"
          variant="reference"
          :active-section="activeGlobalSection"
        />
      </div>
    </div>
  </section>

  <section v-else class="run-reference-shell">
    <RunHeaderStrip />
    <div class="run-current-strip">
      <span>{{ t('commenter.header.current') }}</span>
      <strong class="mono">{{ commenterStore.state.selected_run_detail?.run.current_file ?? t('commenter.idle') }}</strong>
    </div>
    <RunDetailPanel variant="reference" />
  </section>
</template>

<style scoped>
.review-reference-shell {
  min-height: 100vh;
  background: var(--aco-bg);
}

.review-reference-header {
  border-bottom: 1px solid var(--aco-border);
  padding: 18px 22px;
}

.review-title-row {
  display: flex;
  align-items: flex-start;
  gap: 10px;
}

.review-title-row h1,
.review-title-row p {
  margin: 0;
}

.review-title-row h1 {
  color: var(--aco-text);
  font-size: 17px;
}

.review-title-row p {
  margin-top: 4px;
  color: var(--aco-muted);
  font-size: 12px;
  line-height: 1.5;
}

.review-title-icon {
  flex: 0 0 auto;
  margin-top: 3px;
  color: var(--aco-yellow);
}

.review-reference-grid {
  display: grid;
  grid-template-columns: minmax(360px, 0.9fr) minmax(460px, 1.1fr);
  gap: 14px;
  padding: 18px 22px 24px;
}

@media (max-width: 1080px) {
  .review-reference-grid {
    grid-template-columns: 1fr;
  }
}
</style>
