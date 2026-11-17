<script lang="ts">
  import Button from "$lib/components/Button.svelte";
  import Console from "$lib/components/Console.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { app } from "$lib/app.svelte";
  import { duration } from "$lib/format";
  import { runKey, runs } from "$lib/runs.svelte";
  import { INSTALL_COMMAND, isActive, type DepsStatus, type Project } from "$lib/types";

  // Are this project's packages installed? One button to install them, with the
  // install's output right here (it's also in the Logs tab as "install packages").
  let { project, deps }: { project: Project; deps: DepsStatus } = $props();

  const run = $derived(runs.runOf(project.id, INSTALL_COMMAND));
  const installing = $derived(isActive(run?.state));
  const failed = $derived(run?.state === "failed");
  const busy = $derived(runKey(project.id, INSTALL_COMMAND) in runs.busy);
  let showLog = $state(false);
  // Open the log while installing and when it fails; leave it as the user set it otherwise.
  $effect(() => {
    if (installing || failed) showLog = true;
  });

  const status = $derived.by(() => {
    if (installing) return { tone: "busy", label: "Installing…", text: `Running ${deps.command}.` };
    if (failed) return { tone: "bad", label: "Install failed", text: "See the log below for what went wrong." };
    switch (deps.state) {
      case "installed":
        return { tone: "ok", label: "Installed", text: `Up to date with ${deps.lockfile ?? "package.json"}.` };
      case "missing":
        return { tone: "warn", label: "Not installed", text: "No node_modules yet, so the project won't run." };
      case "outdated":
        return {
          tone: "warn",
          label: "Out of date",
          text: `${deps.lockfile ?? "The lockfile"} changed after the last install (a pull may have added packages).`,
        };
    }
  });

  function install() {
    showLog = true;
    runs.start(project.id, INSTALL_COMMAND).then(() => app.reload());
  }
</script>

<section class="packages" aria-label="Packages">
  <div class="top">
    <span class="eyebrow">Packages</span>
    <span class="pill {status.tone}">
      {#if status.tone === "busy"}<span class="spinner" aria-hidden="true"></span>{/if}
      {status.label}
    </span>
    <span class="mgr mono">{deps.manager}</span>
    <span class="spacer"></span>
    {#if installing}
      {#if run?.startedAt}<span class="up mono">{duration(runs.now - run.startedAt)}</span>{/if}
      <button class="stop" disabled={busy || run?.state === "stopping"} onclick={() => runs.stop(project.id, INSTALL_COMMAND)}
        >■ Stop</button
      >
    {:else}
      <Button
        variant={deps.state === "installed" && !failed ? "secondary" : "primary"}
        onclick={install}
        disabled={busy}
        title="Runs {deps.command} in the project folder"
      >
        <Icon name={deps.state === "installed" && !failed ? "refresh" : "download"} size={14} />
        {failed ? "Try again" : deps.state === "missing" ? "Install" : deps.state === "outdated" ? "Update" : "Reinstall"}
      </Button>
    {/if}
  </div>
  <p class="text">
    {status.text}
    {#if run && !installing}
      <button class="link" onclick={() => (showLog = !showLog)}>{showLog ? "Hide log" : "Show log"}</button>
    {/if}
  </p>
  {#if showLog && run}
    <div class="log">
      <Console projectId={project.id} commandId={INSTALL_COMMAND} live={installing} />
    </div>
  {/if}
</section>

<style>
  .packages {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-7) var(--space-8);
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
  }
  .top {
    display: flex;
    align-items: center;
    gap: var(--space-5);
  }
  .eyebrow {
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .pill {
    display: inline-flex;
    align-items: center;
    gap: var(--space-3);
    padding: 2px 9px;
    border-radius: var(--radius-pill);
    font-size: 12.5px;
    font-weight: 600;
  }
  .pill.ok {
    background: var(--success-pill);
    color: var(--success);
  }
  .pill.warn {
    background: var(--warning-panel);
    color: var(--accent-fg);
    border: 1px solid var(--warning-border);
  }
  .pill.bad {
    background: var(--error-pill);
    color: var(--error-text);
  }
  .pill.busy {
    background: var(--bg-hover);
    color: var(--text);
  }
  .mgr {
    font-size: 12px;
    color: var(--text-faint);
  }
  .spacer {
    flex: 1;
  }
  .up {
    font-size: 12px;
    color: var(--text-muted);
  }
  .text {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-muted);
  }
  .link {
    margin-left: var(--space-3);
    border: 0;
    padding: 0;
    background: transparent;
    color: var(--accent-fg);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .stop {
    height: 36px;
    padding: 0 var(--space-8);
    border: 1px solid var(--error-border);
    border-radius: var(--radius-button);
    background: var(--error-pill);
    color: var(--error-text);
    font-weight: 500;
    white-space: nowrap;
    cursor: pointer;
  }
  .stop:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .spinner {
    width: 10px;
    height: 10px;
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
  .log :global(.console) {
    height: 240px;
  }
</style>
