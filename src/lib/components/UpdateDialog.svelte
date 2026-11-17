<script lang="ts">
  import { onMount } from "svelte";
  import { runs } from "$lib/runs.svelte";
  import { updater } from "$lib/updater.svelte";
  import { shortDate } from "$lib/format";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";

  // DESIGN.md "Dialog". One place for every update state: found, downloading,
  // installing, up to date, failed, and "not available in dev builds".
  let first = $state<HTMLDivElement>();
  const busy = $derived(updater.status === "downloading" || updater.status === "installing");
  const running = $derived(runs.active.length);

  onMount(() => {
    first?.querySelector("button")?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !busy) updater.open = false;
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  /** Release notes are Markdown from CHANGELOG.md; show them as plain lines. */
  const lines = $derived(
    updater.notes
      .split("\n")
      .map((l) => l.trim())
      .filter((l) => l !== "" && !/^#+\s*\[?v?\d/.test(l)),
  );
  const mb = (n: number) => (n / 1048576).toFixed(1);
</script>

<div class="scrim">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="upd-title">
    {#if updater.status === "unsupported"}
      <h2 id="upd-title">Updates work in installed builds</h2>
      <p>This is a development build, so there's nothing to update. Installed copies check GitHub Releases on start and every few hours.</p>
    {:else if updater.status === "current" || updater.status === "checking"}
      <h2 id="upd-title">{updater.status === "checking" ? "Checking for updates…" : "DevDock is up to date"}</h2>
      <p>You have version <span class="mono">{updater.current ?? "?"}</span>, the latest release.</p>
    {:else if updater.status === "error"}
      <h2 id="upd-title" class="bad"><Icon name="alert" size={16} /> Update didn't finish</h2>
      <p>{updater.error}</p>
      <p class="muted">Your current version keeps working. You can try again, or download the installer from GitHub Releases.</p>
    {:else}
      <h2 id="upd-title">DevDock {updater.version} is ready</h2>
      <p>
        You have <span class="mono">{updater.current ?? "?"}</span>.
        {#if updater.date}Released {shortDate(Date.parse(updater.date))}.{/if}
        Installing restarts DevDock; Windows asks for permission once because DevDock lives in Program Files.
      </p>
      {#if lines.length}
        <div class="notes selectable">
          {#each lines as l, i (i)}
            {#if /^#+\s/.test(l)}
              <div class="group">{l.replace(/^#+\s*/, "")}</div>
            {:else}
              <div class="line">{l.replace(/^[-*]\s*/, "")}</div>
            {/if}
          {/each}
        </div>
      {/if}
      {#if running > 0 && !busy}
        <p class="warn">
          <Icon name="alert" size={13} />
          {running === 1 ? "1 process DevDock started" : `${running} processes DevDock started`} will be stopped.
        </p>
      {/if}
      {#if busy}
        <div class="progress" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={updater.progress ? Math.round(updater.progress * 100) : undefined}>
          <div class="bar" class:indeterminate={updater.progress === null || updater.status === "installing"} style="width: {(updater.progress ?? 0.3) * 100}%"></div>
        </div>
        <p class="muted mono small">
          {updater.status === "installing"
            ? "Starting the installer…"
            : updater.total
              ? `${mb(updater.received)} of ${mb(updater.total)} MB`
              : `${mb(updater.received)} MB`}
        </p>
      {/if}
    {/if}

    <div class="actions">
      {#if updater.status === "available"}
        <Button variant="ghost" onclick={() => updater.skip()}>Skip this version</Button>
        <span class="spacer"></span>
        <div bind:this={first}><Button variant="secondary" onclick={() => (updater.open = false)}>Later</Button></div>
        <Button variant="primary" onclick={() => updater.install()}>Install and restart</Button>
      {:else if busy}
        <Button variant="secondary" disabled>Installing…</Button>
      {:else if updater.status === "error"}
        <div bind:this={first}><Button variant="secondary" onclick={() => (updater.open = false)}>Close</Button></div>
        <Button variant="primary" onclick={() => updater.check(false)}>Try again</Button>
      {:else}
        <div bind:this={first}><Button variant="secondary" onclick={() => (updater.open = false)}>Close</Button></div>
      {/if}
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
    width: min(500px, calc(100vw - 48px));
    padding: 22px;
    background: var(--bg-overlay);
    border: 1px solid var(--line-overlay);
    border-radius: var(--radius-dialog);
    box-shadow: var(--shadow-dialog);
    animation: dd-overlay 180ms var(--ease);
  }
  h2 {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    margin: 0;
    font-size: 19px;
    font-weight: 700;
    text-wrap: balance;
  }
  h2.bad {
    color: var(--error-text);
  }
  p {
    margin: 0;
    color: var(--text-soft);
    line-height: 1.55;
  }
  .muted {
    color: var(--text-muted);
  }
  .small {
    font-size: 12px;
  }
  .notes {
    max-height: 220px;
    overflow-y: auto;
    padding: var(--space-5) var(--space-6);
    background: var(--bg-app);
    border-radius: var(--radius-button);
    font-size: 13px;
  }
  .group {
    margin: var(--space-4) 0 var(--space-2);
    font-weight: 600;
  }
  .group:first-child {
    margin-top: 0;
  }
  .line {
    position: relative;
    padding: var(--space-1) 0 var(--space-1) var(--space-6);
    color: var(--text-soft);
  }
  .line::before {
    content: "";
    position: absolute;
    left: 2px;
    top: 0.75em;
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: var(--text-faint);
  }
  .warn {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    font-size: 13px;
    color: var(--accent-fg);
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
  .actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-4);
    margin-top: var(--space-4);
  }
  .spacer {
    flex: 1;
  }
</style>
