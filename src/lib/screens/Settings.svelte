<script lang="ts">
  import { onMount } from "svelte";
  import { agents } from "$lib/agents.svelte";
  import { app } from "$lib/app.svelte";
  import Button from "$lib/components/Button.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { clockTime } from "$lib/format";
  import type { AgentId, TerminalKind } from "$lib/types";
  import DataSettings from "./settings/DataSettings.svelte";
  import { theme, type ThemePref } from "$lib/theme.svelte";
  import { updater } from "$lib/updater.svelte";
  import { relativeTime } from "$lib/format";
  import type { IconName } from "$lib/components/Icon.svelte";

  const THEMES: { pref: ThemePref; label: string; icon: IconName }[] = [
    { pref: "system", label: "Windows", icon: "monitor" },
    { pref: "light", label: "Light", icon: "sun" },
    { pref: "dark", label: "Dark", icon: "moon" },
  ];

  // T6.5 first pass: editor, terminal, default agent; what DevDock found on PATH.
  onMount(() => {
    agents.load();
  });

  const ov = $derived(agents.overview);
</script>

<section class="screen">
  <div class="inner">
    <header>
      <div class="eyebrow">Preferences</div>
      <h1>Settings</h1>
    </header>

    <section class="panel">
      <div class="row">
        <div>
          <div class="label">Appearance</div>
          <div class="help">
            {theme.pref === "system"
              ? `Follows Windows, currently ${theme.resolved}.`
              : `Always ${theme.pref}, whatever Windows uses.`}
          </div>
        </div>
        <div class="segmented" role="radiogroup" aria-label="Theme">
          {#each THEMES as t (t.pref)}
            <button
              role="radio"
              aria-checked={theme.pref === t.pref}
              class:on={theme.pref === t.pref}
              onclick={() => theme.set(t.pref)}
            >
              <Icon name={t.icon} size={14} />
              {t.label}
            </button>
          {/each}
        </div>
      </div>
    </section>

    {#if ov}
      <section class="panel">
        <div class="row">
          <div>
            <div class="label">Editor</div>
            <div class="help">Used by "Open in editor". Must be on PATH.</div>
          </div>
          <select value={ov.settings.editor} onchange={(e) => agents.setSettings({ editor: e.currentTarget.value })}>
            {#each ov.editors as ed (ed.command)}
              <option value={ed.command}>{ed.name}{ed.path ? "" : " (not found)"}</option>
            {/each}
          </select>
        </div>
        <div class="row">
          <div>
            <div class="label">Default terminal</div>
            <div class="help">Preselected in each project's Agents tab.</div>
          </div>
          <select
            value={ov.settings.terminal}
            onchange={(e) => agents.setSettings({ terminal: e.currentTarget.value as TerminalKind })}
          >
            {#each ov.terminals as t (t.kind)}
              <option value={t.kind}>{t.name}{t.available ? "" : " (not found)"}</option>
            {/each}
          </select>
        </div>
        <div class="row">
          <div>
            <div class="label">Default agent</div>
            <div class="help">Marked DEFAULT in the Agents tab.</div>
          </div>
          <select
            value={ov.settings.agent}
            onchange={(e) => agents.setSettings({ agent: e.currentTarget.value as AgentId })}
          >
            {#each ov.agents as a (a.id)}
              <option value={a.id}>{a.name}{a.path ? "" : " (not installed)"}</option>
            {/each}
          </select>
        </div>
      </section>

      <section class="panel">
        <div class="panel-head">
          <span class="title">Detected tools</span>
          <span class="help">checked {clockTime(ov.tools.checkedAt)}</span>
          <span class="spacer"></span>
          <Button variant="secondary" onclick={() => agents.recheck()} disabled={agents.checking}>
            <Icon name="refresh" size={13} /> {agents.checking ? "Checking…" : "Re-check"}
          </Button>
        </div>
        <table>
          <tbody>
            {#each ov.tools.tools as t (t.id)}
              <tr>
                <td class="name">{t.name}</td>
                <td class="mono small">{t.id}</td>
                <td class="mono small path" class:missing={!t.path} title={t.path ?? ""}>
                  {t.path ?? "not found on PATH"}
                </td>
              </tr>
            {/each}
            <tr>
              <td class="name">Git Bash</td>
              <td class="mono small">git-bash</td>
              <td class="mono small path" class:missing={!ov.tools.gitBash}>{ov.tools.gitBash ?? "not found next to Git"}</td>
            </tr>
          </tbody>
        </table>
      </section>

      <DataSettings />

      <section class="panel">
        <div class="row">
          <div>
            <div class="label">DevDock <span class="mono version">{updater.current ?? ""}</span></div>
            <div class="help">
              {#if updater.status === "unsupported"}
                Development build. Installed copies update from GitHub Releases.
              {:else if updater.status === "checking"}
                Checking GitHub Releases…
              {:else if updater.status === "available"}
                Version {updater.version} is available.
              {:else if updater.status === "error"}
                {updater.error}
              {:else if updater.checkedAt}
                Up to date. Checked {relativeTime(updater.checkedAt)}.
              {:else}
                Checks GitHub Releases on start and every 6 hours.
              {/if}
            </div>
          </div>
          {#if updater.status === "available"}
            <Button variant="primary" onclick={() => (updater.open = true)}>
              <Icon name="download" size={13} /> Update to {updater.version}
            </Button>
          {:else}
            <Button variant="secondary" onclick={() => updater.check(false)} disabled={updater.status === "checking"}>
              <Icon name="refresh" size={13} /> Check for updates
            </Button>
          {/if}
        </div>
      </section>
    {:else}
      <p class="help">Loading editors, terminals and agents…</p>
    {/if}
  </div>
</section>

<style>
  .screen {
    height: 100%;
    overflow-y: auto;
    padding: var(--space-12) var(--space-14);
  }
  /* DESIGN.md section 5: Settings content is capped at 760 px and centred. */
  .inner {
    display: flex;
    flex-direction: column;
    gap: var(--space-9);
    max-width: 760px;
    margin: 0 auto;
  }
  h1 {
    margin: var(--space-1) 0 0;
    font-size: 28px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  .panel {
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-8);
    padding: var(--space-7) var(--space-8);
    border-top: 1px solid var(--line);
  }
  .row:first-child {
    border-top: 0;
  }
  .label {
    font-weight: 600;
  }
  .version {
    margin-left: var(--space-2);
    font-size: 12px;
    font-weight: 400;
    color: var(--text-muted);
  }
  .help {
    font-size: 12.5px;
    color: var(--text-muted);
  }
  select {
    min-width: 220px;
    height: 36px;
    padding: 0 var(--space-5);
    background: var(--bg-app);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-button);
    color: var(--text);
    font: inherit;
  }
  select:focus {
    outline: none;
    border-color: var(--accent-fg);
  }
  .segmented {
    display: inline-flex;
    padding: 3px;
    background: var(--bg-app);
    border: 1px solid var(--line);
    border-radius: var(--radius-row);
  }
  .segmented button {
    display: inline-flex;
    align-items: center;
    gap: var(--space-3);
    height: 30px;
    padding: 0 var(--space-6);
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--text-muted);
    font-size: 13px;
    cursor: pointer;
    transition:
      background var(--t-fast) var(--ease),
      color var(--t-fast) var(--ease);
  }
  .segmented button:hover {
    color: var(--text);
  }
  .segmented button.on {
    background: var(--bg-hover);
    color: var(--text);
    font-weight: 600;
  }
  .panel-head {
    display: flex;
    align-items: center;
    gap: var(--space-6);
    padding: var(--space-6) var(--space-8);
    border-bottom: 1px solid var(--line);
  }
  .title {
    font-weight: 600;
  }
  .spacer {
    flex: 1;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 14px;
    table-layout: fixed;
  }
  td {
    padding: var(--space-4) var(--space-8);
    border-top: 1px solid var(--line);
  }
  tr:first-child td {
    border-top: 0;
  }
  td.name {
    width: 170px;
  }
  .small {
    font-size: 12px;
  }
  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-soft);
  }
  .path.missing {
    color: var(--text-faint);
  }
</style>
