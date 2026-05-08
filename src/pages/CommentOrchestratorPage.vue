<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { Box, Globe2 } from 'lucide-vue-next';

import DiffToolSettingsPanel from '../components/commenter/DiffToolSettingsPanel.vue';
import ProjectProfilesPanel from '../components/commenter/ProjectProfilesPanel.vue';
import RunHeaderStrip from '../components/commenter/RunHeaderStrip.vue';
import RunDetailPanel from '../components/commenter/RunDetailPanel.vue';
import { commenterStore } from '../lib/commenterStore';
import { use_messages } from '../locales/messages';

const { t } = use_messages();

type WorkspaceMode = 'project' | 'run' | 'global';
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
      <nav class="global-subnav" aria-label="全局设置分区">
        <a href="#api-credentials">{{ t('global.section.apiCredentials') }}</a>
        <a href="#concurrency-quota">{{ t('global.section.concurrencyQuota') }}</a>
        <a href="#diff-tool">{{ t('global.section.diffTool') }}</a>
        <a href="#storage-logs">{{ t('global.section.storageLogs') }}</a>
        <a href="#about-settings">{{ t('global.section.about') }}</a>
      </nav>
      <div class="global-reference-content">
        <DiffToolSettingsPanel ref="diffSettingsPanel" variant="reference" />
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
