<script lang="ts">
  import Checkbox from "$lib/components/Checkbox.svelte";
  import Select from "$lib/components/Select.svelte";
  import { agents } from "$lib/agents.svelte";
  import { app } from "$lib/app.svelte";
  import * as api from "$lib/api";
  import Button from "$lib/components/Button.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { plural } from "$lib/format";
  import { runs } from "$lib/runs.svelte";
  import { spaces } from "$lib/spaces.svelte";

  // T8.4: scan roots & depth, safety toggles, data file export / import.
  type Preview = Awaited<ReturnType<typeof api.importPreview>>;
  let pending = $state<{ path: string; preview: Preview } | null>(null);
  let busy = $state(false);
  /** The "Clear history" confirmation is open. */
  let clearing = $state(false);
  let alsoPorts = $state(false);

  async function confirmClear() {
    await app.clearAllHistory(alsoPorts);
    await runs.refreshList();
    clearing = false;
    alsoPorts = false;
  }
  const settings = $derived(agents.overview?.settings);

  async function setDepth(path: string, depth: number) {
    try {
      await api.setScanRootDepth(path, depth);
      app.error = null;
    } catch (err) {
      app.error = api.toErrorDto(err);
    }
    await app.reload();
  }

  async function exportData() {
    const path = await api.pickSaveFile("devdock-export.json");
    if (!path) return;
    try {
      await api.exportData(path);
      agents.showToast(`Exported to ${path}`, "ok");
    } catch (err) {
      app.error = api.toErrorDto(err);
    }
  }

  async function chooseImport() {
    const path = await api.pickJsonFile();
    if (!path) return;
    try {
      pending = { path, preview: await api.importPreview(path) };
      app.error = null;
    } catch (err) {
      app.error = api.toErrorDto(err);
    }
  }

  async function confirmImport() {
    if (!pending) return;
    busy = true;
    try {
      await api.importData(pending.path);
      pending = null;
      app.error = null;
      app.selectedId = null;
      await Promise.all([app.reload(), agents.load(), spaces.load(), runs.refreshList()]);
      agents.showToast("Imported. The previous data is kept as devdock.json.bak.", "ok");
    } catch (err) {
      app.error = api.toErrorDto(err);
    } finally {
      busy = false;
    }
  }
</script>

<section class="panel">
  <div class="head">
    <span class="title">Scan roots</span>
    <span class="spacer"></span>
    <Button variant="secondary" onclick={() => app.rescan()} disabled={app.scanning || !app.lib?.scanRoots.length}>
      <Icon name="refresh" size={13} /> Rescan
    </Button>
    <Button variant="secondary" onclick={() => app.addScanRoot()} disabled={app.scanning}>
      <Icon name="plus" size={13} /> Add
    </Button>
  </div>
  {#each app.lib?.scanRoots ?? [] as root (root.path)}
    <div class="row">
      <span class="mono small path" title={root.path}>{root.path}</span>
      <label class="depth">
        depth
        <Select
          value={root.depth}
          mono
          width="64px"
          ariaLabel="Scan depth for {root.path}"
          options={[1, 2, 3, 4, 5, 6].map((d) => ({ value: d, label: String(d) }))}
          onchange={(d) => setDepth(root.path, d)}
        />
      </label>
      <button class="icon" aria-label="Remove scan root {root.path}" title="Stop scanning this folder" onclick={() => app.removeScanRoot(root.path)}>
        <Icon name="x" size={13} />
      </button>
    </div>
  {:else}
    <p class="pad help">No scan roots. Add a folder such as D:\dev and DevDock finds the projects in it.</p>
  {/each}
</section>

<section class="panel">
  <div class="head"><span class="title">Safety</span></div>
  <label class="row toggle locked">
    <Checkbox variant="switch" checked disabled />
    <span>
      Ask before ending a process DevDock didn't start
      <span class="help block">Always on. The dialog shows the command, folder, start time and children first.</span>
    </span>
  </label>
  <label class="row toggle">
    <Checkbox
      variant="switch"
      checked={settings?.checkPorts ?? true}
      onchange={(v) => agents.setSettings({ checkPorts: v })}
    />
    <span>
      Check a command's port before starting it
      <span class="help block">Shows the conflict banner instead of starting when the port is taken.</span>
    </span>
  </label>
  <label class="row toggle">
    <Checkbox
      variant="switch"
      checked={settings?.autoPort ?? false}
      onchange={(v) => agents.setSettings({ autoPort: v })}
    />
    <span>
      Give commands their own port
      <span class="help block"
        >Commands without a port setting get a free port, picked once and saved, instead of 3000 or 5173.</span
      >
    </span>
  </label>
  <label class="row toggle locked">
    <Checkbox variant="switch" checked disabled />
    <span>
      Stop DevDock's processes when it closes
      <span class="help block">Always on: each run lives in a Windows job that ends with DevDock. Closing asks first.</span>
    </span>
  </label>
</section>

<section class="panel">
  <div class="head">
    <span class="title">Data</span>
    <span class="spacer"></span>
    <Button variant="secondary" onclick={exportData}>Export…</Button>
    <Button variant="secondary" onclick={chooseImport}>Import…</Button>
  </div>
  <div class="row">
    <span class="help">Saved to</span>
    <span class="mono small selectable path">{app.lib?.savedTo ?? ""}</span>
  </div>
  {#if pending}
    <div class="confirm" role="alertdialog" aria-labelledby="import-title">
      <div id="import-title" class="confirm-title">Replace your data with this file?</div>
      <p>
        {plural(pending.preview.currentProjects, "project")} will be replaced by {plural(pending.preview.projects, "project")},
        {plural(pending.preview.workspaces, "workspace")}, {plural(pending.preview.scanRoots, "scan root")} and
        {plural(pending.preview.shortcuts, "shortcut")} from <span class="mono">{pending.path}</span>. Folders on disk are not
        touched; the current data is kept as <span class="mono">devdock.json.bak</span>.
      </p>
      <div class="actions">
        <Button variant="secondary" onclick={() => (pending = null)}>Cancel</Button>
        <button class="danger" disabled={busy} onclick={confirmImport}>{busy ? "Importing…" : "Replace and import"}</button>
      </div>
    </div>
  {/if}
  <div class="row">
    <span class="grow">
      History
      <span class="help block"
        >Recent activity and agent launches for every project, and the finished runs on the Running screen.</span
      >
    </span>
    <Button variant="secondary" onclick={() => (clearing = !clearing)}>Clear history…</Button>
  </div>
  {#if clearing}
    <div class="confirm" role="alertdialog" aria-labelledby="clear-title">
      <div id="clear-title" class="confirm-title">Clear history for every project?</div>
      <p>
        Activity, launch history and finished runs go. Projects, commands, saved ports, addresses and settings stay.
        Commands that are running keep running.
      </p>
      <label class="also">
        <Checkbox bind:checked={alsoPorts} />
        Also forget ports learned from earlier runs (they're checked for conflicts before starting)
      </label>
      <div class="actions">
        <Button variant="secondary" onclick={() => (clearing = false)}>Cancel</Button>
        <button class="danger" onclick={confirmClear}>Clear history</button>
      </div>
    </div>
  {/if}
</section>

<style>
  .grow {
    flex: 1;
    min-width: 0;
  }
  .also {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    margin-bottom: var(--space-6);
    font-size: 13px;
    color: var(--text-soft);
    cursor: pointer;
  }
  .panel {
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-6) var(--space-8);
    border-bottom: 1px solid var(--line);
  }
  .title {
    font-weight: 600;
  }
  .spacer {
    flex: 1;
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--space-6);
    padding: var(--space-5) var(--space-8);
    border-top: 1px solid var(--line);
  }
  .head + .row,
  .head + label.row {
    border-top: 0;
  }
  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-soft);
  }
  .small {
    font-size: 12.5px;
  }
  .depth {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    font-size: 12.5px;
    color: var(--text-muted);
  }
  .icon {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }
  .icon:hover {
    color: var(--error-text);
    background: var(--bg-hover);
  }
  .toggle {
    align-items: flex-start;
    cursor: pointer;
  }
  .locked {
    cursor: default;
  }
  .help {
    font-size: 12.5px;
    color: var(--text-muted);
  }
  .block {
    display: block;
    margin-top: var(--space-1);
  }
  .pad {
    margin: 0;
    padding: var(--space-6) var(--space-8);
  }
  .confirm {
    margin: 0 var(--space-8) var(--space-8);
    padding: var(--space-7);
    background: var(--warning-panel);
    border: 1px solid var(--warning-border);
    border-radius: var(--radius-row);
  }
  .confirm-title {
    font-weight: 600;
  }
  .confirm p {
    margin: var(--space-3) 0 var(--space-6);
    color: var(--text-soft);
    overflow-wrap: anywhere;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-4);
  }
  .danger {
    height: 36px;
    padding: 0 var(--space-8);
    border: 0;
    border-radius: var(--radius-button);
    background: var(--error);
    color: var(--error-ink);
    font-weight: 600;
    cursor: pointer;
  }
  .danger:disabled {
    opacity: 0.6;
  }
</style>
