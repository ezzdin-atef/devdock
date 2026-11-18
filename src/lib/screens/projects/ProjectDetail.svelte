<script lang="ts">
  import { app } from "$lib/app.svelte";
  import Button from "$lib/components/Button.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { KIND_LABEL } from "$lib/types";
  import { runs } from "$lib/runs.svelte";
  import AgentsTab from "./AgentsTab.svelte";
  import OverviewTab from "./OverviewTab.svelte";
  import GitTab from "./GitTab.svelte";
  import FilesTab from "./FilesTab.svelte";
  import { files } from "$lib/files.svelte";
  import RunTab from "./RunTab.svelte";
  import LogsTab from "./LogsTab.svelte";
  import Home from "./Home.svelte";
  import RenameInput from "./RenameInput.svelte";
  import Logo from "$lib/components/Logo.svelte";
  import PathLine from "$lib/components/PathLine.svelte";

  const total = $derived(app.lib?.counts.all ?? 0);
</script>

<section
  class="detail"
  class:fill={app.selected && (app.tab === "run" || app.tab === "logs" || app.tab === "files") && !app.selected.missing}
  class:wide={!app.selected || app.tab === "logs" || app.tab === "files"}
>
  {#if app.selected}
    {@const project = app.selected}
    <button class="back-link" onclick={() => app.select(null)} title="Back to all projects (Esc)">
      <span class="back-icon"><Icon name="chevron" size={13} /></span> All projects
    </button>
    <header>
      <div class="eyebrow">{KIND_LABEL[project.kind]}{project.isGit ? " · git" : ""}</div>
      <div class="title-row">
        {#if app.renamingId === project.id}
          <RenameInput {project} large />
        {:else}
          <h1 ondblclick={() => app.startRename(project.id)} title="Double-click to rename">
            {project.name}
          </h1>
        {/if}
        <button
          class="star"
          class:on={project.favorite}
          aria-pressed={project.favorite}
          aria-label={project.favorite ? "Remove from favorites" : "Add to favorites"}
          title={project.favorite ? "Remove from favorites" : "Add to favorites"}
          onclick={() => app.toggleFavorite(project.id, !project.favorite)}
        >
          <Icon name="star" size={18} filled={project.favorite} />
        </button>
      </div>
      <PathLine path={project.path} />
    </header>

    {@const changes = project.git?.state === "repo" ? project.git.data.changedFiles : 0}
    <div class="tabs" role="tablist">
      <button
        role="tab"
        aria-selected={app.tab === "overview"}
        class:active={app.tab === "overview"}
        onclick={() => (app.tab = "overview")}><Icon name="grid" size={14} /> Overview</button
      >
      <button
        role="tab"
        aria-selected={app.tab === "git"}
        class:active={app.tab === "git"}
        onclick={() => (app.tab = "git")}
      >
        <Icon name="branch" size={14} /> Git
        {#if changes > 0}<span class="tab-count mono" title="{changes} changed files">{changes}</span>{/if}
      </button>
      <button
        role="tab"
        aria-selected={app.tab === "files"}
        class:active={app.tab === "files"}
        onclick={() => (app.tab = "files")}
      >
        <Icon name="file" size={14} /> Files
        {#if files.byProject[project.id]?.open?.dirty}<span class="tab-dot" title="Unsaved changes"></span>{/if}
      </button>
      <button
        role="tab"
        aria-selected={app.tab === "run"}
        class:active={app.tab === "run"}
        onclick={() => (app.tab = "run")}
      >
        <Icon name="play" size={12} /> Run
        {#if runs.activeIn(project.id) > 0}<span class="tab-count mono ok" title="running"
            >{runs.activeIn(project.id)}</span
          >{/if}
      </button>
      <button
        role="tab"
        aria-selected={app.tab === "logs"}
        class:active={app.tab === "logs"}
        onclick={() => (app.tab = "logs")}><Icon name="lines" size={14} /> Logs</button
      >
      <button
        role="tab"
        aria-selected={app.tab === "agents"}
        class:active={app.tab === "agents"}
        onclick={() => (app.tab = "agents")}><Icon name="terminal" size={14} /> Agents</button
      >
    </div>

    {#key `${project.id}:${app.tab}`}
    <div class="tab-body">
    {#if app.tab === "git"}
      <GitTab {project} />
    {:else if app.tab === "files" && !project.missing}
      <FilesTab {project} />
    {:else if app.tab === "run"}
      <RunTab {project} />
    {:else if app.tab === "logs"}
      <LogsTab {project} />
    {:else if app.tab === "agents"}
      <AgentsTab {project} />
    {:else}

    {#if project.missing}
      <div class="missing-panel">
        <div class="missing-title"><Icon name="alert" size={15} /> Folder not found</div>
        <p>
          DevDock can't find this folder anymore. If it moved, point DevDock at the new place; its
          name, favorite and history are kept.
        </p>
        <div class="actions">
          <Button variant="primary" onclick={() => app.locate(project.id)}>
            <Icon name="folder" size={14} /> Locate…
          </Button>
          <button class="destructive" onclick={() => app.remove(project.id)}>
            Remove from DevDock
          </button>
        </div>
        <p class="note">Removing only forgets the entry; nothing on disk is touched.</p>
      </div>
    {:else}
      <OverviewTab {project} />
    {/if}
    {/if}
    </div>
    {/key}
  {:else if total === 0 && app.lib}
    <div class="welcome">
      <Logo size={56} />
      <h1>Dock your first project</h1>
      <p class="lead">
        DevDock keeps your local projects in one place: their Git state, their dev servers, the ports
        they hold and the agents you run in them. Nothing leaves this machine.
      </p>
      <div class="actions">
        <Button variant="primary" onclick={() => app.addProject()}>
          <Icon name="folder" size={14} /> Choose a project folder…
        </Button>
        <Button variant="secondary" onclick={() => app.addScanRoot()}>
          <Icon name="search" size={14} /> Scan a parent folder
        </Button>
      </div>
      <ol class="steps">
        <li>
          <span class="step-title">Add one folder, or a whole root</span>
          <span class="step-text"
            >A scan root such as <span class="mono">D:\dev</span> finds every project inside it, skipping
            <span class="mono">node_modules</span> and build output.</span
          >
        </li>
        <li>
          <span class="step-title">See what each project is doing</span>
          <span class="step-text"
            >Branch, uncommitted files, running commands and ports, read straight from Git and Windows.</span
          >
        </li>
        <li>
          <span class="step-title">Jump anywhere with <span class="kbd mono">Ctrl K</span></span>
          <span class="step-text"
            >Open a project in your editor, start <span class="mono">dev</span>, or launch an agent
            without leaving the keyboard.</span
          >
        </li>
      </ol>
    </div>
  {:else}
    <Home />
  {/if}
</section>

<style>
  .detail {
    display: flex;
    flex-direction: column;
    gap: var(--space-9);
    max-width: 980px;
  }
  .title-row {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    margin: var(--space-1) 0 var(--space-3);
  }
  h1 {
    margin: 0;
    font-size: 28px;
    font-weight: 700;
    letter-spacing: -0.02em;
    overflow-wrap: anywhere;
    cursor: text;
  }
  .star {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    flex-shrink: 0;
    border: 0;
    border-radius: var(--radius-button);
    background: transparent;
    color: var(--icon);
    cursor: pointer;
  }
  .star:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
  .star.on {
    color: var(--accent-fg);
  }
  header {
    min-width: 0;
  }
  /* Run tab: the page stops scrolling; the command list and the log split the
     remaining height and scroll on their own. */
  .detail.fill {
    flex: 1;
    min-height: 560px;
  }
  /* Logs use the full width of the window: long lines need it. */
  .detail.wide {
    max-width: none;
  }
  .detail.fill .tab-body {
    flex: 1;
    min-height: 0;
  }
  .tab-body {
    display: flex;
    flex-direction: column;
    gap: var(--space-9);
    animation: dd-enter 200ms var(--ease);
  }
  .tabs {
    display: flex;
    gap: var(--space-10);
    border-bottom: 1px solid var(--line);
    margin-bottom: calc(-1 * var(--space-2));
  }
  .tabs button {
    display: inline-flex;
    align-items: center;
    gap: var(--space-3);
    height: 40px;
    padding: 0;
    border: 0;
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    background: transparent;
    color: var(--text-muted);
    font-size: 14.5px;
    cursor: pointer;
  }
  .tabs button {
    transition:
      color var(--t-fast) var(--ease),
      border-color var(--t-fast) var(--ease);
  }
  .tabs button:hover {
    color: var(--text);
  }
  .tabs button :global(svg) {
    flex-shrink: 0;
    color: var(--icon);
    opacity: 0.8;
    transition: color var(--t-fast) var(--ease), opacity var(--t-fast) var(--ease);
  }
  .tabs button:hover :global(svg) {
    opacity: 1;
  }
  .tabs button.active :global(svg) {
    color: var(--accent-fg);
    opacity: 1;
  }
  .tabs button.active {
    color: var(--text);
    font-weight: 600;
    border-bottom-color: var(--accent-fg);
  }
  .tab-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent-fg);
  }
  .tab-count.ok {
    background: var(--success-pill);
    color: var(--success);
  }
  .tab-count {
    font-size: 11px;
    padding: 1px 6px;
    border-radius: var(--radius-pill);
    background: var(--accent-tint);
    color: var(--accent-fg);
    font-weight: 400;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: var(--space-5);
  }
  .missing-panel {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    padding: var(--space-8);
    background: var(--error-panel);
    border: 1px solid var(--error-border);
    border-radius: var(--radius-panel);
  }
  .missing-title {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    color: var(--error-text);
    font-size: 16px;
    font-weight: 600;
  }
  .missing-panel p {
    margin: 0;
    color: var(--text-soft);
  }
  .missing-panel .note {
    color: var(--text-muted);
    font-size: 12.5px;
  }
  .destructive {
    height: 36px;
    padding: 0 var(--space-8);
    border: 1px solid var(--error-border);
    border-radius: var(--radius-button);
    background: transparent;
    color: var(--error-text);
    font-weight: 500;
    cursor: pointer;
  }
  .destructive:hover {
    background: var(--error-pill);
  }
  /* First run: logo, promise, two ways in, what to expect. */
  .welcome {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-8);
    max-width: 560px;
    padding-top: clamp(24px, 8vh, 96px);
    animation: dd-enter 320ms var(--ease);
  }
  .welcome h1 {
    margin: var(--space-4) 0 0;
    cursor: default;
    text-wrap: balance;
  }
  .lead {
    margin: 0;
    font-size: 15px;
    line-height: 1.6;
    color: var(--text-soft);
    text-wrap: pretty;
  }
  .steps {
    list-style: none;
    counter-reset: step;
    margin: var(--space-8) 0 0;
    padding: var(--space-12) 0 0;
    border-top: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
    width: 100%;
  }
  .steps li {
    counter-increment: step;
    display: grid;
    grid-template-columns: 28px 1fr;
    column-gap: var(--space-4);
    row-gap: var(--space-1);
  }
  .steps li::before {
    content: counter(step);
    grid-row: span 2;
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 20px;
    color: var(--accent-fg);
  }
  .step-title {
    font-weight: 600;
  }
  .step-text {
    font-size: 13px;
    color: var(--text-muted);
    line-height: 1.55;
  }
  .step-text .mono {
    font-size: 12.5px;
    color: var(--text-soft);
  }
  .kbd {
    display: inline-block;
    padding: 1px 6px;
    border: 1px solid var(--line-strong);
    border-bottom-width: 2px;
    border-radius: var(--radius-badge);
    background: var(--bg-panel);
    font-size: 11.5px;
    font-weight: 400;
    color: var(--text-soft);
  }
  .back-link {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    height: 28px;
    margin: calc(var(--space-4) * -1) 0 calc(var(--space-4) * -1) calc(var(--space-3) * -1);
    padding: 0 var(--space-4) 0 var(--space-2);
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-size: 13px;
    cursor: pointer;
  }
  .back-link:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
  .back-icon {
    display: grid;
    transform: rotate(90deg);
  }
</style>
