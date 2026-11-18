<script lang="ts">
  import { app } from "$lib/app.svelte";
  import DragGhost from "$lib/components/DragGhost.svelte";
  import ErrorBanner from "$lib/components/ErrorBanner.svelte";
  import LibrarySidebar from "./projects/LibrarySidebar.svelte";
  import ProjectDetail from "./projects/ProjectDetail.svelte";
  import { tick } from "svelte";

  // The project list and a project's page share this scroller. Opening a project
  // starts at its top; going back to the list returns to where you were in it.
  let main: HTMLElement;
  let listScroll = 0;
  let shown: string | null = null;

  // `.pre`: runs before the DOM swaps list <-> project, so the list's position is
  // read before the shorter project page clamps it.
  $effect.pre(() => {
    const id = app.selectedId;
    if (id === shown || !main) return;
    const fromList = shown === null;
    shown = id;
    if (id !== null) {
      if (fromList) listScroll = main.scrollTop;
      main.scrollTo({ top: 0 });
    } else {
      tick().then(() => main.scrollTo({ top: listScroll }));
    }
  });
</script>

<div class="screen">
  <LibrarySidebar />
  <section class="main" bind:this={main}>
    {#if app.notice}
      <ErrorBanner error={app.notice} tone="warning" ondismiss={() => (app.notice = null)} />
    {/if}
    {#if app.error}
      <ErrorBanner error={app.error} ondismiss={() => app.dismissError()} />
    {/if}
    <ProjectDetail />
  </section>
</div>

<DragGhost />

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
