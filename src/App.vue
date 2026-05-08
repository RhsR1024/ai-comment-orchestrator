<script setup lang="ts">
import { RouterView } from 'vue-router';

import Sidebar from './components/Sidebar.vue';
</script>

<template>
  <div class="app-shell app-shell--reference">
    <Sidebar />
    <main class="app-main">
      <RouterView />
    </main>
  </div>
</template>
