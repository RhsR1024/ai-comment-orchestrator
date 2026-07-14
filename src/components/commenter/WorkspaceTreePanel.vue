<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import { AlertTriangle } from 'lucide-vue-next';

import { commenterStore } from '../../lib/commenterStore';
import { commenterApi } from '../../lib/tauri';
import type { CommenterDirEntry, CommenterJobStatus } from '../../lib/commenterTypes';
import { use_messages } from '../../locales/messages';
import WorkspaceTreeNode, { type WorkspaceTreeRenderNode } from './WorkspaceTreeNode.vue';

type TreeNode = WorkspaceTreeRenderNode;

const MAX_CACHED_DIRECTORIES = 128;
const directory_cache = new Map<string, CommenterDirEntry[]>();

function directoryCacheKey(profile: string, relative_path: string): string {
  const current_profile = commenterStore.state.profiles.find(
    (entry) => entry.project_key === profile
  );
  const version = current_profile
    ? `${current_profile.root_path}\u0000${current_profile.updated_at}`
    : '';
  return `${profile}\u0000${version}\u0000${relative_path}`;
}

function cachedDirectory(profile: string, relative_path: string): CommenterDirEntry[] | null {
  const key = directoryCacheKey(profile, relative_path);
  const entries = directory_cache.get(key);
  if (!entries) {
    return null;
  }
  directory_cache.delete(key);
  directory_cache.set(key, entries);
  return entries;
}

function cacheDirectory(profile: string, relative_path: string, entries: CommenterDirEntry[]) {
  const key = directoryCacheKey(profile, relative_path);
  directory_cache.delete(key);
  directory_cache.set(key, entries);
  while (directory_cache.size > MAX_CACHED_DIRECTORIES) {
    const oldest = directory_cache.keys().next().value;
    if (oldest === undefined) {
      break;
    }
    directory_cache.delete(oldest);
  }
}

function invalidateProfileCache(profile: string) {
  const prefix = `${profile}\u0000`;
  for (const key of directory_cache.keys()) {
    if (key.startsWith(prefix)) {
      directory_cache.delete(key);
    }
  }
}

const emit = defineEmits<{ 'select-file': [string] }>();

const { t } = use_messages();

const detail = computed(() => commenterStore.state.selected_run_detail);
const profile_key = computed(() => detail.value?.run.profile_key ?? null);
const current_file = computed(() => detail.value?.run.current_file ?? null);
const jobs_ref = computed(() => detail.value?.jobs ?? null);

const active_job_statuses = new Set<CommenterJobStatus>([
  'leased',
  'requesting',
  'validating',
  'writing',
  'retry_waiting'
]);

const queued_paths = computed(() => {
  const jobs = jobs_ref.value;
  if (!jobs) return new Set<string>();
  const next = new Set<string>();
  for (const job of jobs) {
    next.add(job.relative_path);
  }
  return next;
});

const job_status_by_path = computed(() => {
  const jobs = jobs_ref.value;
  const next = new Map<string, CommenterJobStatus>();
  if (!jobs) return next;
  for (const job of jobs) {
    next.set(job.relative_path, job.status);
  }
  return next;
});

const active_paths = computed(() => {
  const next = new Set<string>();
  const run_key = detail.value?.run.run_key ?? null;
  if (current_file.value) {
    next.add(current_file.value);
  }
  for (const job of jobs_ref.value ?? []) {
    if (active_job_statuses.has(job.status)) {
      next.add(job.relative_path);
    }
  }
  if (run_key) {
    const stream_prefix = `${run_key}|`;
    for (const [key, slice] of commenterStore.state.live_streams) {
      if (slice.status === 'streaming' && key.startsWith(stream_prefix)) {
        next.add(key.slice(stream_prefix.length));
      }
    }
  }
  return next;
});

const active_path_key = computed(() => [...active_paths.value].sort().join('\n'));

const root = reactive<TreeNode>({
  kind: 'dir',
  name: '',
  relative_path: '',
  children: null,
  expanded: true,
  loading: false,
  error: null
});

const root_error = ref<string | null>(null);

function mapEntry(entry: CommenterDirEntry): TreeNode {
  return {
    kind: entry.kind,
    name: entry.name,
    relative_path: entry.relative_path,
    children: null,
    expanded: false,
    loading: false,
    error: null
  };
}

async function load(node: TreeNode) {
  const profile = profile_key.value;
  if (!profile) {
    return;
  }
  if (node.children) {
    return;
  }
  node.loading = true;
  node.error = null;
  if (node === root) {
    root_error.value = null;
  }
  try {
    const cached = cachedDirectory(profile, node.relative_path);
    const entries = cached ?? await commenterApi.listDir(profile, node.relative_path);
    if (!cached) {
      cacheDirectory(profile, node.relative_path, entries);
    }
    node.children = entries
      .slice()
      .sort((left, right) => {
        if (left.kind !== right.kind) {
          return left.kind === 'dir' ? -1 : 1;
        }
        return left.name.localeCompare(right.name);
      })
      .map(mapEntry);
  } catch (error) {
    node.error = error instanceof Error ? error.message : String(error);
    if (node === root) {
      root_error.value = node.error;
    }
  } finally {
    node.loading = false;
  }
}

async function reloadRoot() {
  if (profile_key.value) {
    invalidateProfileCache(profile_key.value);
  }
  root.children = null;
  root.error = null;
  root_error.value = null;
  await load(root);
}

async function toggle(node: TreeNode) {
  if (node.kind === 'file') {
    if (queued_paths.value.has(node.relative_path)) {
      emit('select-file', node.relative_path);
    }
    return;
  }

  node.expanded = !node.expanded;
  if (node.expanded && !node.children) {
    await load(node);
  }
}

async function expandAncestors(path: string) {
  const segments = path.split('/').filter(Boolean);
  let cursor: TreeNode = root;
  let traversed = '';

  for (const segment of segments) {
    if (!cursor.children) {
      await load(cursor);
    }
    if (!cursor.children) {
      return;
    }
    traversed = traversed ? `${traversed}/${segment}` : segment;
    const next = cursor.children.find((child) => child.relative_path === traversed);
    if (!next) {
      return;
    }
    if (next.kind === 'dir') {
      next.expanded = true;
      if (!next.children) {
        await load(next);
      }
    }
    cursor = next;
  }
}

async function expandActivePaths(paths: Set<string>) {
  if (!profile_key.value || paths.size === 0) {
    return;
  }
  root.expanded = true;
  for (const path of paths) {
    await expandAncestors(path);
  }
}

watch(
  () => profile_key.value,
  async (key) => {
    root.children = null;
    root.expanded = true;
    root.error = null;
    root_error.value = null;
    if (key) {
      await load(root);
      await expandActivePaths(active_paths.value);
    }
  },
  { immediate: true }
);

watch(
  () => active_path_key.value,
  async () => {
    await expandActivePaths(active_paths.value);
  },
  { immediate: true }
);
</script>

<template>
  <section class="workspace-tree-panel">
    <header class="workspace-tree-header">
      <div>
        <h3>{{ t('commenter.detail') }}</h3>
        <p>{{ t('commenter.detail.help') }}</p>
      </div>
    </header>

    <div
      v-if="root_error"
      class="tree-error"
    >
      <div class="tree-error-copy">
        <AlertTriangle :size="14" />
        <span>{{ t('commenter.tree.error.root') }}</span>
      </div>
      <small>{{ root_error }}</small>
      <button
        type="button"
        class="tree-retry"
        @click="reloadRoot"
      >
        {{ t('commenter.tree.retry') }}
      </button>
    </div>

    <div
      v-else-if="!profile_key || queued_paths.size === 0"
      class="empty-state"
    >
      {{ t('commenter.tree.empty') }}
    </div>

    <ul
      v-else
      class="tree"
    >
      <WorkspaceTreeNode
        v-for="child in root.children ?? []"
        :key="child.relative_path"
        :node="child"
        :queued-paths="queued_paths"
        :active-paths="active_paths"
        :job-status-by-path="job_status_by_path"
        :current-file="current_file"
        @toggle="toggle"
      />
    </ul>
  </section>
</template>

<style scoped>
.workspace-tree-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  background: rgba(5, 15, 18, 0.5);
}

.workspace-tree-header {
  padding: 12px 14px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}

.workspace-tree-header h3,
.workspace-tree-header p {
  margin: 0;
}

.workspace-tree-header p {
  margin-top: 4px;
  color: #88a5a1;
  font-size: 12px;
}

.tree {
  list-style: none;
  margin: 0;
  padding: 8px;
  overflow: auto;
  flex: 1;
  min-height: 0;
}

.tree-error {
  display: grid;
  gap: 8px;
  padding: 14px;
  color: #fca5a5;
}

.tree-error-copy {
  display: flex;
  align-items: center;
  gap: 8px;
}

.tree-error small {
  color: #9db5b1;
  font-family:
    ui-monospace,
    SFMono-Regular,
    Menlo,
    Monaco,
    Consolas,
    "Liberation Mono",
    monospace;
}

.tree-retry {
  justify-self: start;
  padding: 6px 10px;
  border-radius: 8px;
  border: 1px solid rgba(239, 68, 68, 0.25);
  background: rgba(239, 68, 68, 0.08);
  color: #fee2e2;
  cursor: pointer;
}
</style>
