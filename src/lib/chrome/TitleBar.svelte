<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { app } from "$lib/app.svelte";
  import { palette } from "$lib/palette.svelte";
  import { runs } from "$lib/runs.svelte";
  import { theme } from "$lib/theme.svelte";
  import { updater } from "$lib/updater.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Logo from "$lib/components/Logo.svelte";
  import type { View } from "$lib/types";

  // Custom title bar (the native one is off: decorations=false in tauri.conf.json).
  // Three columns so the palette trigger sits at the true centre of the window:
  // brand + current screen | palette | status, theme, window controls.
  // Empty space carries data-tauri-drag-region: drag to move, double-click to maximize.

  const VIEW_LABEL: Record<View, string> = {
    projects: "Projects",
    spaces: "Spaces",
    running: "Running",
    ports: "Ports",
    agents: "Agents",
    settings: "Settings",
  };

  const win = getCurrentWindow();
  const count = $derived(runs.active.length);
  const next = $derived(theme.resolved === "dark" ? "light" : "dark");
  const context = $derived(
    app.view === "projects" && app.selected ? app.selected.name : null,
  );

  let maximized = $state(false);
  let focused = $state(true);

  onMount(() => {
    const sync = () => win.isMaximized().then((m) => (maximized = m)).catch(() => {});
    sync();
    const offResize = win.onResized(sync);
    const offFocus = win.onFocusChanged(({ payload }) => (focused = payload));
    return () => {
      offResize.then((f) => f());
      offFocus.then((f) => f());
    };
  });

  // Each call goes through Tauri; `close` raises CloseRequested, so the
  // "stop running processes?" dialog still appears exactly as before.
  const minimize = () => win.minimize().catch(() => {});
  const toggleMax = () => win.toggleMaximize().catch(() => {});
  const close = () => win.close().catch(() => {});
</script>

<header class="title-bar" class:blurred={!focused} data-tauri-drag-region>
  <div class="left" data-tauri-drag-region>
    <span class="brand" data-tauri-drag-region>
      <Logo size={22} />
      <span class="wordmark" data-tauri-drag-region>DevDock</span>
    </span>
    <span class="crumbs" data-tauri-drag-region>
      <span class="slash" aria-hidden="true">/</span>
      <span class="crumb" data-tauri-drag-region>{VIEW_LABEL[app.view]}</span>
      {#if context}
        <span class="slash" aria-hidden="true">/</span>
        <span class="crumb current" data-tauri-drag-region title={context}>{context}</span>
      {/if}
    </span>
  </div>

  <button class="trigger" onclick={() => palette.show()} title="Command palette (Ctrl K)">
    <Icon name="search" size={14} />
    <span class="hint">Jump to a project, command or port…</span>
    <span class="key mono">Ctrl K</span>
  </button>

  <div class="right" data-tauri-drag-region>
    {#if updater.showPill}
      <button class="update mono" title="DevDock {updater.version} is ready to install" onclick={() => (updater.open = true)}>
        <Icon name="download" size={12} />
        {updater.status === "available" ? `Update ${updater.version}` : "Updating…"}
      </button>
    {/if}
    <button
      class="running mono"
      class:on={count > 0}
      title="Show everything DevDock is running"
      onclick={() => (app.view = "running")}
    >
      <span class="dot"></span>{count} running
    </button>
    <button
      class="icon-btn"
      aria-label="Switch to {next} theme"
      title="Switch to {next} theme{theme.pref === 'system' ? ' (now following Windows)' : ''}"
      onclick={() => theme.toggle()}
    >
      {#key theme.resolved}
        <span class="theme-icon"><Icon name={theme.resolved === "dark" ? "moon" : "sun"} size={15} /></span>
      {/key}
    </button>

    <div class="controls">
      <button class="ctl" aria-label="Minimize" title="Minimize" onclick={minimize}>
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M0 5h10" /></svg>
      </button>
      <button
        class="ctl"
        aria-label={maximized ? "Restore" : "Maximize"}
        title={maximized ? "Restore down" : "Maximize"}
        onclick={toggleMax}
      >
        {#if maximized}
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
            <path d="M2.5 2.5V.5h7v7h-2M.5 2.5h7v7h-7z" />
          </svg>
        {:else}
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M.5.5h9v9h-9z" /></svg>
        {/if}
      </button>
      <button class="ctl close" aria-label="Close" title="Close" onclick={close}>
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M0 0l10 10M10 0 0 10" /></svg>
      </button>
    </div>
  </div>
</header>

<style>
  .title-bar {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
    column-gap: var(--space-8);
    height: var(--title-bar-h);
    padding-left: var(--space-8);
    background: var(--bg-chrome);
    border-bottom: 1px solid var(--line);
    user-select: none;
    transition: color 160ms var(--ease);
  }
  /* Windows convention: an inactive window's title bar recedes. */
  .title-bar.blurred .brand,
  .title-bar.blurred .crumbs,
  .title-bar.blurred .controls {
    opacity: 0.55;
  }
  .title-bar.blurred .trigger,
  .title-bar.blurred .running,
  .title-bar.blurred .icon-btn {
    opacity: 0.75;
  }
  .left,
  .right {
    display: flex;
    align-items: center;
    height: 100%;
    min-width: 0;
  }
  .left {
    gap: var(--space-5);
  }
  .right {
    justify-content: flex-end;
    gap: var(--space-4);
  }

  .brand {
    display: inline-flex;
    align-items: center;
    gap: var(--space-4);
    flex-shrink: 0;
    transition: opacity 160ms var(--ease);
  }
  .wordmark {
    font-size: 15px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .crumbs {
    display: inline-flex;
    align-items: center;
    gap: var(--space-3);
    min-width: 0;
    font-size: 14px;
    color: var(--text-muted);
    transition: opacity 160ms var(--ease);
  }
  .slash {
    color: var(--line-strong);
    font-weight: 300;
  }
  .crumb {
    white-space: nowrap;
  }
  .crumb.current {
    color: var(--text-soft);
    font-weight: 500;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .trigger {
    display: flex;
    align-items: center;
    gap: var(--space-5);
    width: clamp(260px, 34vw, 440px);
    height: 32px;
    padding: 0 var(--space-4) 0 var(--space-6);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-button);
    background: var(--bg-panel);
    color: var(--text-muted);
    font-size: 13px;
    cursor: pointer;
    transition:
      border-color var(--t-fast) var(--ease),
      color var(--t-fast) var(--ease),
      opacity 160ms var(--ease);
  }
  .trigger:hover {
    border-color: var(--text-faint);
    color: var(--text-soft);
  }
  .trigger :global(svg) {
    color: var(--icon);
  }
  .hint {
    flex: 1;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .key {
    padding: 1px 6px;
    border: 1px solid var(--line-strong);
    border-bottom-width: 2px;
    border-radius: var(--radius-badge);
    font-size: 11px;
  }

  .running {
    display: inline-flex;
    align-items: center;
    gap: var(--space-3);
    height: 26px;
    padding: 0 var(--space-5);
    border: 1px solid var(--line);
    border-radius: var(--radius-pill);
    background: transparent;
    font-size: 12px;
    color: var(--text-muted);
    white-space: nowrap;
    cursor: pointer;
    transition:
      border-color var(--t-fast) var(--ease),
      opacity 160ms var(--ease);
  }
  .running:hover {
    border-color: var(--line-strong);
  }
  .update {
    display: inline-flex;
    align-items: center;
    gap: var(--space-3);
    height: 26px;
    padding: 0 var(--space-5);
    border: 0;
    border-radius: var(--radius-pill);
    background: var(--accent);
    color: var(--accent-ink);
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
    cursor: pointer;
    animation: dd-overlay 220ms var(--ease);
  }
  .update:hover {
    filter: brightness(1.08);
  }
  .running .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    border: 1.5px solid var(--idle);
  }
  .running.on {
    border-color: var(--success-border);
    background: var(--success-pill);
    color: var(--success);
  }
  .running.on .dot {
    border: 0;
    background: var(--success);
  }

  .icon-btn {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border: 0;
    border-radius: var(--radius-button);
    background: transparent;
    color: var(--icon);
    cursor: pointer;
    transition:
      background var(--t-fast) var(--ease),
      color var(--t-fast) var(--ease),
      opacity 160ms var(--ease);
  }
  .icon-btn:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
  .theme-icon {
    display: grid;
    animation: spin-in 320ms var(--ease);
  }
  @keyframes spin-in {
    from {
      opacity: 0;
      transform: rotate(-60deg) scale(0.7);
    }
  }

  /* Window controls: full-height, flush to the edge, Windows-sized hit targets
     (46 px) so they behave like the native ones users already aim for. */
  .controls {
    display: flex;
    align-self: stretch;
    margin-left: var(--space-3);
    border-left: 1px solid var(--line);
    transition: opacity 160ms var(--ease);
  }
  .ctl {
    display: grid;
    place-items: center;
    width: 46px;
    height: 100%;
    border: 0;
    border-radius: 0;
    background: transparent;
    color: var(--text-soft);
    cursor: default;
    transition:
      background 100ms var(--ease),
      color 100ms var(--ease);
  }
  .ctl svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1;
    shape-rendering: crispEdges;
  }
  .ctl:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
  .ctl:active {
    transform: none;
    background: var(--bg-raised);
  }
  /* The one fixed red: Windows' close-hover colour, the same in both themes. */
  .ctl.close:hover {
    background: #c42b1c;
    color: #fff;
  }
  .ctl.close:active {
    background: #b0281a;
    color: #fff;
  }
  .ctl:focus-visible {
    outline-offset: -2px;
  }
</style>
