<script lang="ts">
  import { app } from "$lib/app.svelte";
  import ErrorBanner from "$lib/components/ErrorBanner.svelte";
  import LibrarySidebar from "./projects/LibrarySidebar.svelte";
  import ProjectDetail from "./projects/ProjectDetail.svelte";
  import ProjectList from "./projects/ProjectList.svelte";
</script>

<div class="screen">
  <LibrarySidebar />
  <ProjectList />
  <section class="main">
    {#if app.notice}
      <ErrorBanner error={app.notice} tone="warning" ondismiss={() => (app.notice = null)} />
    {/if}
    {#if app.error}
      <ErrorBanner error={app.error} ondismiss={() => app.dismissError()} />
    {/if}
    <ProjectDetail />
  </section>
</div>

<style>
  .screen {
    display: flex;
    height: 100%;
    min-height: 0;
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-9);
    padding: var(--space-12) var(--space-14);
    overflow-y: auto;
  }
</style>
