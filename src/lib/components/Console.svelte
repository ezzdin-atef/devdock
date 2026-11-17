<script lang="ts">
  import { tick } from "svelte";
  import * as api from "$lib/api";
  import { logTime } from "$lib/format";
  import { runKey, runs } from "$lib/runs.svelte";
  import type { CommandId, LogLine, ProjectId } from "$lib/types";

  // DESIGN.md "Console / log": mono, timestamps faint, stderr lines tinted with ERR.
  let {
    projectId,
    commandId,
    live,
  }: { projectId: ProjectId; commandId: CommandId; live: boolean } = $props();

  const MAX = 5000; // same as the Rust ring buffer
  let lines = $state<LogLine[]>([]);
  let truncated = $state(false);
  let filter = $state<"all" | "out" | "err">("all");
  let follow = $state(true);
  let box: HTMLDivElement | undefined = $state();
  let nextSeq = 0;
  let loading = false;
  /** An update arrived while a fetch was running: fetch again when it ends. */
  let again = false;
  let currentKey = "";

  const key = $derived(runKey(projectId, commandId));
  const shown = $derived(
    filter === "all" ? lines : lines.filter((l) => l.stream === filter || l.stream === "system"),
  );

  async function fetchMore(reset: boolean) {
    if (loading && !reset) {
      again = true;
      return;
    }
    loading = true;
    again = false;
    const forKey = key;
    if (reset) {
      lines = [];
      nextSeq = 0;
      truncated = false;
    }
    const page = await api.runLogs(projectId, commandId, nextSeq).catch(() => null);
    loading = false;
    if (forKey !== key || !page) return; // switched commands meanwhile
    if (again) queueMicrotask(() => fetchMore(false));
    if (page.truncated && nextSeq > 0) truncated = true;
    nextSeq = page.nextSeq;
    if (page.lines.length) {
      const merged = lines.concat(page.lines);
      lines = merged.length > MAX ? merged.slice(merged.length - MAX) : merged;
      if (follow) {
        await tick();
        box?.scrollTo({ top: box.scrollHeight });
      }
    }
  }

  // One effect for both triggers so they can't race: a new command starts over,
  // a new tick for the same command fetches only the lines after `nextSeq`.
  $effect(() => {
    const k = key;
    const ticked = runs.logTick[k];
    if (k !== currentKey) {
      currentKey = k;
      fetchMore(true);
    } else if (ticked !== undefined) {
      fetchMore(false);
    }
  });

  function onScroll() {
    if (!box) return;
    const atBottom = box.scrollHeight - box.scrollTop - box.clientHeight < 40;
    if (!atBottom && follow) follow = false;
    if (atBottom && !follow) follow = true;
  }

  async function setFollow(on: boolean) {
    follow = on;
    if (on) {
      await tick();
      box?.scrollTo({ top: box.scrollHeight });
    }
  }
</script>

<div class="console-wrap">
  <div class="toolbar">
    <div class="segmented" role="radiogroup" aria-label="Show">
      {#each [["all", "All"], ["out", "Output"], ["err", "Errors"]] as [value, label] (value)}
        <button
          role="radio"
          aria-checked={filter === value}
          class:on={filter === value}
          onclick={() => (filter = value as typeof filter)}>{label}</button
        >
      {/each}
    </div>
    <label class="follow">
      <input type="checkbox" checked={follow} onchange={(e) => setFollow(e.currentTarget.checked)} />
      Follow
    </label>
  </div>

  <div class="console mono" bind:this={box} onscroll={onScroll} role="log" aria-live="off">
    {#if truncated}
      <div class="line system">… older lines were dropped (DevDock keeps the last {MAX})</div>
    {/if}
    {#each shown as l (l.seq)}
      <div class="line {l.stream}">
        <span class="ts">{logTime(l.ts)}</span>
        {#if l.stream === "err"}<span class="tag">ERR</span>{/if}
        <span class="text selectable">{l.text}</span>
      </div>
    {:else}
      <div class="line system">No output yet.</div>
    {/each}
    {#if live && follow}<span class="cursor" aria-hidden="true">▍</span>{/if}
  </div>
</div>

<style>
  .console-wrap {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
    overflow: hidden;
  }
  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-4) var(--space-5);
    background: var(--bg-panel);
    border-bottom: 1px solid var(--line);
  }
  .segmented {
    display: inline-flex;
    padding: 3px;
    background: var(--bg-app);
    border: 1px solid var(--line);
    border-radius: var(--radius-row);
  }
  .segmented button {
    height: 24px;
    padding: 0 var(--space-6);
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--text-muted);
    font-size: 12px;
    cursor: pointer;
  }
  .segmented button.on {
    background: var(--bg-hover);
    color: var(--text);
    font-weight: 600;
  }
  .follow {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    font-size: 12px;
    color: var(--text-muted);
    cursor: pointer;
  }
  .follow input {
    accent-color: var(--accent-fg);
  }
  /* Fixed height so the console scrolls (and Follow works) instead of growing
     the whole page with every line. */
  .console {
    height: clamp(240px, 46vh, 640px);
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
  .line.err {
    background: var(--error-log);
    color: var(--error-text);
  }
  .tag {
    flex-shrink: 0;
    font-size: 9.5px;
    padding: 0 5px;
    align-self: center;
    line-height: 16px;
    border-radius: var(--radius-badge);
    background: var(--error-pill);
    color: var(--error);
  }
  .line.system .text,
  .line.system {
    color: var(--text-muted);
  }
  .cursor {
    display: inline-block;
    margin-left: var(--space-6);
    color: var(--text-muted);
    animation: blink 1s steps(1) infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .cursor {
      animation: none;
    }
  }
</style>
