<script lang="ts">
  import { onMount, tick } from "svelte";
  import Button from "$lib/components/Button.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { duration } from "$lib/format";
  import { runs } from "$lib/runs.svelte";
  import { security } from "$lib/security.svelte";

  // A scanner install's live log: each step, the commands DevDock runs, their
  // output, and the download's progress. Stays readable after a failure.
  const tool = $derived(security.logOpen ?? "");
  const log = $derived(security.installLogs[tool]);
  const running = $derived(log?.state === "running");
  const name = $derived(log?.name ?? security.scanners?.find((s) => s.id === tool)?.name ?? tool);

  let box = $state<HTMLDivElement>();
  let first = $state<HTMLDivElement>();
  let follow = $state(true);
  let copied = $state(false);

  function close() {
    security.logOpen = null;
  }

  onMount(() => {
    first?.querySelector("button")?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  // Keep the newest line in view, unless the user scrolled up to read.
  $effect(() => {
    void log?.lines.length;
    if (follow && box) tick().then(() => box && (box.scrollTop = box.scrollHeight));
  });

  function onScroll() {
    if (!box) return;
    follow = box.scrollHeight - box.scrollTop - box.clientHeight < 24;
  }

  async function copyLog() {
    if (!log) return;
    try {
      await navigator.clipboard.writeText(log.lines.join("\n"));
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      // Clipboard blocked.
    }
  }

  const kind = (line: string) => (line.startsWith("== ") ? "step" : line.startsWith("> ") ? "cmd" : "out");
  const pct = $derived(log?.progress != null ? Math.round(log.progress * 100) : null);
  const elapsed = $derived(log ? (log.finishedAt ?? runs.now) - log.startedAt : 0);
</script>

<div class="scrim" role="presentation" onclick={(e) => e.target === e.currentTarget && close()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="install-log-title">
    <header>
      <h2 id="install-log-title">
        {#if log?.state === "failed"}<span class="bad"><Icon name="alert" size={16} /></span>{/if}
        {#if log?.state === "done"}<span class="ok"><Icon name="check" size={16} /></span>{/if}
        {running ? `Installing ${name}` : `${name} install`}
      </h2>
      <span class="when mono">{log ? duration(elapsed) : ""}</span>
    </header>

    {#if log}
      <div class="status" class:bad={log.state === "failed"} class:ok={log.state === "done"}>
        {#if running}<span class="spinner" aria-hidden="true"></span>{/if}
        <span>{log.state === "failed" ? (log.error ?? "The install failed.") : log.stage}{running ? "…" : ""}</span>
        {#if running && pct !== null}<span class="mono pct">{pct}%</span>{/if}
      </div>
      {#if running}
        <div class="progress" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={pct ?? undefined}>
          <div class="bar" class:indeterminate={pct === null} style="width: {pct ?? 30}%"></div>
        </div>
      {/if}

      <div class="log selectable mono" bind:this={box} onscroll={onScroll} aria-live="polite">
        {#if log.dropped}<div class="out faint">… {log.dropped} earlier lines not kept</div>{/if}
        {#each log.lines as line, i (i)}
          <div class={kind(line)}>{kind(line) === "step" ? line.slice(3) : line}</div>
        {:else}
          <div class="out faint">Waiting for output…</div>
        {/each}
      </div>
    {:else}
      <p class="muted">No install of {name} has run since DevDock started.</p>
    {/if}

    <div class="actions">
      {#if log}
        <Button variant="ghost" onclick={copyLog}>
          <Icon name={copied ? "check" : "copy"} size={14} />
          {copied ? "Copied" : "Copy log"}
        </Button>
      {/if}
      <span class="spacer"></span>
      {#if running}
        <button class="stop" onclick={() => security.cancelInstall(tool)}>Cancel install</button>
      {/if}
      <div bind:this={first}>
        <Button variant={running ? "secondary" : "primary"} onclick={close}>{running ? "Hide" : "Close"}</Button>
      </div>
    </div>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    background: var(--scrim);
    animation: dd-fade 160ms var(--ease);
  }
  .dialog {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    width: min(760px, calc(100vw - 48px));
    max-height: calc(100vh - 96px);
    padding: 22px;
    background: var(--bg-overlay);
    border: 1px solid var(--line-overlay);
    border-radius: var(--radius-dialog);
    box-shadow: var(--shadow-dialog);
    animation: dd-overlay 180ms var(--ease);
  }
  header {
    display: flex;
    align-items: baseline;
    gap: var(--space-5);
  }
  h2 {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    flex: 1;
    margin: 0;
    font-size: 19px;
    font-weight: 700;
  }
  .when {
    font-size: 12px;
    color: var(--text-muted);
  }
  .ok {
    color: var(--success);
  }
  .bad {
    color: var(--error-text);
  }
  .muted {
    margin: 0;
    color: var(--text-muted);
  }
  .status {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    font-size: 13.5px;
    color: var(--text);
  }
  .pct {
    margin-left: auto;
    font-size: 12.5px;
    color: var(--text-muted);
  }
  .progress {
    height: 6px;
    overflow: hidden;
    background: var(--bg-hover);
    border-radius: var(--radius-pill);
  }
  .bar {
    height: 100%;
    background: var(--accent);
    border-radius: inherit;
    transition: width 200ms var(--ease);
  }
  .bar.indeterminate {
    width: 30% !important;
    animation: slide 1.1s var(--ease) infinite;
  }
  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(340%);
    }
  }
  .spinner {
    width: 11px;
    height: 11px;
    flex-shrink: 0;
    border: 1.5px solid var(--line-strong);
    border-top-color: var(--accent-fg);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .log {
    height: 340px;
    min-height: 160px;
    overflow: auto;
    padding: var(--space-5) var(--space-6);
    background: var(--bg-app);
    border: 1px solid var(--line);
    border-radius: var(--radius-button);
    font-size: 12px;
    line-height: 1.6;
  }
  .log div {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .log .step {
    margin-top: var(--space-4);
    font-family: var(--font-sans, inherit);
    font-weight: 600;
    color: var(--text);
  }
  .log .step:first-child {
    margin-top: 0;
  }
  .log .cmd {
    color: var(--text-muted);
  }
  .log .out {
    color: var(--text-soft);
  }
  .log .faint {
    color: var(--text-faint);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }
  .spacer {
    flex: 1;
  }
  .stop {
    height: 36px;
    padding: 0 var(--space-8);
    border: 1px solid var(--error-border);
    border-radius: var(--radius-button);
    background: var(--error-pill);
    color: var(--error-text);
    font-weight: 500;
    cursor: pointer;
  }
</style>
