import { createMemoryHistory, createRouter, createWebHistory } from 'vue-router';

const history = typeof window === 'undefined' ? createMemoryHistory() : createWebHistory();
const commenter_page = () => import('../pages/CommentOrchestratorPage.vue');
type CommenterWorkspaceMode = 'project' | 'run' | 'global';

const router = createRouter({
  history,
  routes: [
    {
      path: '/',
      redirect: '/settings'
    },
    {
      path: '/console',
      redirect: '/workspace'
    },
    {
      path: '/tasks',
      redirect: '/workspace'
    },
    {
      path: '/history',
      redirect: '/workspace'
    },
    {
      path: '/settings',
      component: commenter_page,
      props: {
        workspaceMode: 'project' satisfies CommenterWorkspaceMode
      }
    },
    {
      path: '/workspace',
      component: commenter_page,
      props: {
        workspaceMode: 'run' satisfies CommenterWorkspaceMode
      }
    },
    {
      path: '/global',
      component: commenter_page,
      props: {
        workspaceMode: 'global' satisfies CommenterWorkspaceMode
      }
    },
    {
      path: '/tools',
      redirect: '/settings'
    },
    {
      path: '/tools/comment-orchestrator',
      redirect: '/settings'
    }
  ]
});

export default router;
