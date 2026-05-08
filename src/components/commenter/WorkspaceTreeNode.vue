<script setup lang="ts">
import {
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  CircleDashed,
  FileCode,
  Folder,
  FolderOpen,
  Loader2,
  TriangleAlert,
  XCircle
} from 'lucide-vue-next';
import { computed, nextTick, ref, watch } from 'vue';

import type { CommenterJobStatus } from '../../lib/commenterTypes';

export interface WorkspaceTreeRenderNode {
  kind: 'dir' | 'file';
  name: string;
  relative_path: string;
  children: WorkspaceTreeRenderNode[] | null;
  expanded: boolean;
  loading: boolean;
  error: string | null;
}

const props = defineProps<{
  node: WorkspaceTreeRenderNode;
  queuedPaths: Set<string>;
  activePaths: Set<string>;
  jobStatusByPath: Map<string, CommenterJobStatus>;
  currentFile: string | null;
}>();

const emit = defineEmits<{ toggle: [WorkspaceTreeRenderNode] }>();
const row_ref = ref<HTMLElement | null>(null);
const is_current_node = computed(
  () => props.node.kind === 'file' && props.node.relative_path === props.currentFile
);

function isQueued(path: string): boolean {
  return props.queuedPaths.has(path);
}

function jobStatus(path: string): CommenterJobStatus | null {
  return props.jobStatusByPath.get(path) ?? null;
}

function isActiveFile(path: string): boolean {
  const status = jobStatus(path);
  return (
    path === props.currentFile ||
    props.activePaths.has(path) ||
    status === 'leased' ||
    status === 'requesting' ||
    status === 'validating' ||
    status === 'writing' ||
    status === 'retry_waiting'
  );
}

function statusIcon(path: string) {
  if (isActiveFile(path)) {
    return Loader2;
  }
  switch (jobStatus(path)) {
    case 'done':
      return CheckCircle2;
    case 'review_needed':
      return TriangleAlert;
    case 'failed':
    case 'skipped':
      return XCircle;
    case 'pending':
      return CircleDashed;
    default:
      return null;
  }
}

watch(
  is_current_node,
  async (current) => {
    if (!current) {
      return;
    }
    await nextTick();
    row_ref.value?.scrollIntoView({ block: 'nearest' });
  },
  { immediate: true }
);
</script>

<template>
  <li
    class="tree-node"
    :class="{
      'tree-node--queued': isQueued(node.relative_path),
      'tree-node--active': isActiveFile(node.relative_path),
      'tree-node--current': node.relative_path === currentFile
    }"
  >
    <button
      ref="row_ref"
      type="button"
      class="tree-node-row"
      @click="emit('toggle', node)"
    >
      <component
        :is="node.kind === 'dir' ? (node.expanded ? ChevronDown : ChevronRight) : 'span'"
        v-bind="node.kind === 'dir' ? { size: 12 } : { class: 'tree-node-spacer' }"
      />
      <component
        :is="node.kind === 'dir' ? (node.expanded ? FolderOpen : Folder) : FileCode"
        :size="14"
      />
      <component
        :is="statusIcon(node.relative_path) ?? 'span'"
        v-if="node.kind === 'file'"
        class="tree-node-status"
        :class="{ 'tree-node-status--active': isActiveFile(node.relative_path) }"
        :size="13"
      />
      <span class="tree-node-name">{{ node.name }}</span>
    </button>

    <ul
      v-if="node.expanded && node.children"
      class="tree"
    >
      <WorkspaceTreeNode
        v-for="child in node.children"
        :key="child.relative_path"
        :node="child"
        :queued-paths="queuedPaths"
        :active-paths="activePaths"
        :job-status-by-path="jobStatusByPath"
        :current-file="currentFile"
        @toggle="emit('toggle', $event)"
      />
    </ul>
  </li>
</template>

<style scoped>
.tree {
  list-style: none;
  margin: 0;
  padding-left: 14px;
}

.tree-node-row {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  background: none;
  border: 0;
  padding: 4px 6px;
  color: inherit;
  text-align: left;
  cursor: pointer;
  border-radius: 8px;
  font-family:
    ui-monospace,
    SFMono-Regular,
    Menlo,
    Monaco,
    Consolas,
    "Liberation Mono",
    monospace;
  font-size: 12px;
}

.tree-node-spacer {
  display: inline-block;
  width: 12px;
}

.tree-node:not(.tree-node--queued) > .tree-node-row {
  opacity: 0.5;
}

.tree-node--current > .tree-node-row {
  background: rgba(34, 197, 94, 0.14);
  color: #eff8f7;
}

.tree-node--active:not(.tree-node--current) > .tree-node-row {
  background: rgba(20, 184, 166, 0.1);
  color: #dff7f4;
}

.tree-node-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tree-node-status {
  color: #9de8e1;
  flex: 0 0 auto;
}

.tree-node-status--active {
  animation: tree-status-spin 900ms linear infinite;
}

@keyframes tree-status-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
