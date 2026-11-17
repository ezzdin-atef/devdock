<script lang="ts">
  import { app } from "$lib/app.svelte";
  import Button from "$lib/components/Button.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { KIND_LABEL } from "$lib/types";
  import { runs } from "$lib/runs.svelte";
  import AgentsTab from "./AgentsTab.svelte";
  import OverviewTab from "./OverviewTab.svelte";
  import GitTab from "./GitTab.svelte";
  import RunTab from "./RunTab.svelte";
  import RenameInput from "./RenameInput.svelte";
  import Logo from "$lib/components/Logo.svelte";
  import PathLine from "$lib/components/PathLine.svelte";

  const total = $derived(app.lib?.counts.all ?? 0);
</script>

<section class="detail">
  {#if app.selected}
    {@const project = app.selected}
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
        onclick={() => (app.tab = "overview")}>Overview</button
      >
      <button
        role="tab"
        aria-selected={app.tab === "git"}
        class:active={app.tab === "git"}
        onclick={() => (app.tab = "git")}
      >
        Git
        {#if changes > 0}<span class="tab-count mono" title="{changes} changed files">{changes}</span>{/if}
      </button>
      <button
        role="tab"
        aria-selected={app.tab === "run"}
        class:active={app.tab === "run"}
        onclick={() => (app.tab = "run")}
      >
        Run
        {#if runs.activeIn(project.id) > 0}<span class="tab-count mono ok" title="running"
            >{runs.activeIn(project.id)}</span
          >{/if}
      </button>
      <button
        role="tab"
        aria-selected={app.tab === "agents"}
        class:active={app.tab === "agents"}
        onclick={() => (app.tab = "agents")}>Agents</button
      >
    </div>

    {#key `${project.id}:${app.tab}`}
    <div class="tab-body">
    {#if app.tab === "git"}
      <GitTab {project} />
    {:else if app.tab === "run"}
      <RunTab {project} />
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
  {:else if app.lib}
    <div class="pick">
      <p>Pick a project from the list to see its Git state, commands and ports.</p>
      <p class="keys">
        <span class="kbd mono">↑</span><span class="kbd mono">↓</span> move
        <span class="sep"></span>
        <span class="kbd mono">Enter</span> open
        <span class="sep"></span>
        <span class="kbd mono">Ctrl K</span> jump to anything
      </p>
    </div>
  {/if}
</section>

<style>
  .detail {
    display: flex;
    flex-direction: column;
    gap: var(--space-9);
    max-width: 760px;
  }
  .title-row {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    margin: var(--space-1) 0 var(--space-3);
  }
  h1 {
    margin: 0;
    font-size: 27px;
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
    color: var(--text-muted);
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
    font-size: 13.5px;
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
  .tabs button.active {
    color: var(--text);
    font-weight: 600;
    border-bottom-color: var(--accent-fg);
  }
  .tab-count.ok {
    background: var(--success-pill);
    color: var(--success);
  }
  .tab-count {
    font-size: 10px;
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
    font-size: 15px;
    font-weight: 600;
  }
  .missing-panel p {
    margin: 0;
    color: var(--text-soft);
  }
  .missing-panel .note {
    color: var(--text-muted);
    font-size: 12px;
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
    font-size: 14px;
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
    font-size: 11px;
    line-height: 20px;
    color: var(--accent-fg);
  }
  .step-title {
    font-weight: 600;
  }
  .step-text {
    font-size: 12.5px;
    color: var(--text-muted);
    line-height: 1.55;
  }
  .step-text .mono {
    font-size: 11.5px;
    color: var(--text-soft);
  }
  .kbd {
    display: inline-block;
    padding: 1px 6px;
    border: 1px solid var(--line-strong);
    border-bottom-width: 2px;
    border-radius: var(--radius-badge);
    background: var(--bg-panel);
    font-size: 10.5px;
    font-weight: 400;
    color: var(--text-soft);
  }
  .pick {
    padding-top: clamp(16px, 6vh, 64px);
    color: var(--text-muted);
    animation: dd-enter 240ms var(--ease);
  }
  .pick p {
    margin: 0;
  }
  .pick .keys {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin-top: var(--space-6);
    font-size: 12px;
  }
  .sep {
    width: var(--space-6);
  }
</style>
