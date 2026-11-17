<script lang="ts">
  import { onDestroy, tick } from "svelte";
  import * as api from "$lib/api";
  import { logTime } from "$lib/format";
  import { SERVICE_COLORS, ordered } from "$lib/spaces.svelte";
  import type { MergedLine, WorkspaceView } from "$lib/types";

  // T7.5: every service's output merged by time, labelled and coloured per service.
  let { ws }: { ws: WorkspaceView } = $props();

  const MAX = 5000;
  let lines = $state<MergedLine[]>([]);
  let only = $state<string | null>(null);
  let follow = $state(true);
  let box: HTMLDivElement | undefined = $state();
  let cursors: Record<string, number> = {};
  let loading = false;
  let forId = "";

  const services = $derived(ordered(ws));
  const color = (id: string) => SERVICE_COLORS[Math.max(0, services.findIndex((s) => s.id === id)) % SERVICE_COLORS.length];
  const nameOf = (id: string) => services.find((s) => s.id === id)?.name ?? id;
  const shown = $derived(only ? lines.filter((l) => l.service === only) : lines);

  async function fetchMore() {
    if (loading) return;
    loading = true;
    const id = ws.id;
    try {
      const page = await api.workspaceLogs(id, cursors);
      if (id !== forId) return;
      cursors = page.cursors;
      if (page.lines.length) {
        const merged = lines.concat(page.lines);
        lines = merged.length > MAX ? merged.slice(merged.length - MAX) : merged;
        if (follow) {
          await tick();
          box?.scrollTo({ top: box.scrollHeight });
        }
      }
    } catch {
      // The workspace may have been deleted meanwhile.
    } finally {
      loading = false;
    }
  }

  // New workspace selected: start over. Then poll while anything could be printing.
  $effect(() => {
    if (ws.id !== forId) {
      forId = ws.id;
      cursors = {};
      lines = [];
      only = null;
      fetchMore();
    }
  });
  const timer = setInterval(fetchMore, 1000);
  onDestroy(() => clearInterval(timer));
</script>

<section class="logs">
  <div class="toolbar">
    <div class="chips">
      <button class="chip mono" class:on={only === null} onclick={() => (only = null)}>ALL</button>
      {#each services as s (s.id)}
        <button
          class="chip mono"
          class:on={only === s.id}
          style:--c={color(s.id)}
          onclick={() => (only = only === s.id ? null : s.id)}>{s.name}</button
        >
      {/each}
    </div>
    <label class="follow"><input type="checkbox" bind:checked={follow} /> Follow</label>
  </div>
  <div class="console mono" bind:this={box} role="log">
    {#each shown as l (`${l.service}:${l.seq}`)}
      <div class="line" class:err={l.stream === "err"} class:system={l.stream === "system"}>
        <span class="ts">{logTime(l.ts)}</span>
        <span class="svc" style:color={color(l.service)}>{nameOf(l.service)}</span>
        <span class="text selectable">{l.text}</span>
      </div>
    {:else}
      <div class="line system">No output yet. Start the workspace to see every service here.</div>
    {/each}
  </div>
</section>

<style>
  .logs {
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
    overflow: hidden;
  }
  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-6);
    padding: var(--space-4) var(--space-5);
    background: var(--bg-panel);
    border-bottom: 1px solid var(--line);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-3);
  }
  .chip {
    --c: var(--text-muted);
    height: 24px;
    padding: 0 var(--space-5);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-pill);
    background: transparent;
    color: var(--c);
    font-size: 10.5px;
    cursor: pointer;
  }
  .chip.on {
    border-color: var(--accent-fg);
    background: var(--accent-tint);
  }
  .follow {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    font-size: 12px;
    color: var(--text-muted);
  }
  .follow input {
    accent-color: var(--accent-fg);
  }
  .console {
    height: clamp(220px, 38vh, 520px);
    overflow: auto;
    padding: var(--space-5) 0;
    background: var(--bg-console);
    font-size: 11.5px;
    line-height: 1.9;
  }
  .line {
    display: flex;
    gap: var(--space-5);
    padding: 0 var(--space-6);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .ts {
    color: var(--text-faint);
    flex-shrink: 0;
  }
  .svc {
    flex-shrink: 0;
    min-width: 64px;
  }
  .line.err {
    background: var(--error-log);
  }
  .line.err .text {
    color: var(--error-text);
  }
  .line.system .text,
  .line.system {
    color: var(--text-muted);
  }
</style>
