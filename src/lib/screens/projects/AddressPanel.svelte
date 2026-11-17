<script lang="ts">
  import Select from "$lib/components/Select.svelte";
  import { agents } from "$lib/agents.svelte";
  import { app } from "$lib/app.svelte";
  import * as api from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import { runs } from "$lib/runs.svelte";
  import { isActive, type Project, type RunCommand } from "$lib/types";

  // One place for a project's address: its name, its ending, and whether the port
  // shows. Every change saves right away and the preview shows the result.
  let { project }: { project: Project } = $props();

  const ENDINGS = [
    { suffix: ".localhost", help: "Works right away." },
    { suffix: ".test", help: "Needs a one-time hosts-file entry. DevDock adds it for you." },
  ];

  const projectId = $derived(project.id);
  $effect(() => {
    if (!runs.commandsByProject[projectId]) runs.loadCommands(projectId);
  });
  const list = $derived(runs.commandsByProject[project.id]);
  const settings = $derived(agents.overview?.settings);

  const host = $derived(list?.host ?? project.host);
  const ending = $derived(ENDINGS.find((e) => host.endsWith(e.suffix)) ?? ENDINGS[0]);
  const label = $derived(host.slice(0, host.length - ending.suffix.length));

  let name = $state("");
  let error = $state<string | null>(null);
  let saving = $state(false);
  let copied = $state(false);
  // Follow the saved name unless the user is typing.
  let editing = $state(false);
  $effect(() => {
    if (!editing) name = label;
  });

  async function save(nextLabel: string, suffix: string, hidePort: boolean | null) {
    const domain = nextLabel.trim() ? `${nextLabel.trim()}${suffix}` : "";
    saving = true;
    error = await runs.setAddress(project.id, domain, hidePort);
    saving = false;
    if (!error) editing = false;
  }

  const hideNow = $derived(list?.portless ?? project.hidePort ?? settings?.proxy ?? false);
  const saveName = () => {
    if (name.trim() !== label) save(name, ending.suffix, project.hidePort);
    else editing = false;
  };

  /** The port the address would use: a running command's, else a saved one. */
  function portOf(c: RunCommand): number | null {
    const r = runs.runOf(project.id, c.id) ?? c.run;
    if (r?.portHint && isActive(r.state)) return r.portHint;
    return c.portMode === "default" ? c.knownPort : c.port;
  }
  const running = $derived(
    (list?.commands ?? []).filter((c) => isActive((runs.runOf(project.id, c.id) ?? c.run)?.state)),
  );
  const port = $derived(
    running.map(portOf).find((p) => p != null) ?? (list?.commands ?? []).map(portOf).find((p) => p != null) ?? null,
  );

  const url = $derived.by(() => {
    const at = (scheme: string, p: number, std: number) => `${scheme}://${host}${p === std ? "" : `:${p}`}`;
    if (hideNow && list?.proxy.https) return at("https", list.proxy.https, 443);
    if (hideNow && list?.proxy.http) return at("http", list.proxy.http, 80);
    return `http://${host}:${port ?? "port"}`;
  });
  /** Hide was chosen, but the proxy isn't up, so the address still needs the port. */
  const hideBroken = $derived(hideNow && !list?.proxy.http);
  const canOpen = $derived(running.length > 0 && (port != null || (hideNow && !!list?.proxy.http)));

  async function copy() {
    await navigator.clipboard.writeText(url);
    copied = true;
    setTimeout(() => (copied = false), 1200);
  }

  async function addToHosts() {
    saving = true;
    try {
      await api.hostsSync();
      app.error = null;
    } catch (err) {
      app.error = api.toErrorDto(err);
    }
    saving = false;
    await runs.loadCommands(project.id);
  }
</script>

<section class="address" aria-label="Address">
  <div class="top">
    <span class="eyebrow">Address</span>
    <span class="url mono" class:placeholder={!port && !hideNow} class:broken={hideBroken}>{url}</span>
    <span class="spacer"></span>
    <button class="icon-btn" title={copied ? "Copied" : "Copy address"} onclick={copy}>
      <Icon name={copied ? "check" : "copy"} size={14} />
    </button>
    <button
      class="open"
      disabled={!canOpen}
      title={canOpen ? "Open in the browser" : "Start a command in the Run tab first"}
      onclick={() => runs.openUrl(url)}>Open</button
    >
  </div>

  <div class="fields">
    <label class="field">
      <span class="lbl">Name</span>
      <span class="name-box" class:bad={!!error}>
        <input
          class="mono"
          bind:value={name}
          spellcheck="false"
          aria-label="Address name"
          placeholder="shop"
          oninput={() => (editing = true)}
          onblur={saveName}
          onkeydown={(e) => {
            if (e.key === "Enter") (e.currentTarget as HTMLInputElement).blur();
            if (e.key === "Escape") {
              name = label;
              editing = false;
              error = null;
            }
          }}
        />
        <span class="ending">
          <Select
            value={ending.suffix}
            mono
            variant="quiet"
            ariaLabel="Address ending"
            options={ENDINGS.map((e) => ({ value: e.suffix, label: e.suffix }))}
            onchange={(v) => save(name || label, v, project.hidePort)}
          />
        </span>
      </span>
    </label>

    <div class="field">
      <span class="lbl">Port</span>
      <div class="seg" role="radiogroup" aria-label="Port in the address">
        <button
          role="radio"
          aria-checked={!hideNow}
          class:on={!hideNow}
          disabled={saving}
          onclick={() => save(label, ending.suffix, false)}>Show <span class="mono">:{port ?? "port"}</span></button
        >
        <button
          role="radio"
          aria-checked={hideNow}
          class:on={hideNow}
          disabled={saving}
          onclick={() => save(label, ending.suffix, true)}>Hide</button
        >
      </div>
    </div>
  </div>

  <div class="notes">
    {#if error}
      <p class="bad">{error}</p>
    {:else}
      <p>{ending.help}</p>
    {/if}
    {#if hideNow}
      {#if hideBroken}
        <p class="bad">Can't hide the port: {list?.proxyError ?? "DevDock's proxy isn't running."}</p>
      {:else}
        <p>No port needed. DevDock sends {host} to whatever this project is running.</p>
        {#if settings?.https && !list?.proxy.https}
          <p>For HTTPS, trust DevDock's certificate in Settings first.</p>
        {/if}
      {/if}
    {/if}
    {#if list?.hostsMissing}
      <p class="warn">
        {host} needs a hosts-file entry before it opens.
        <button class="link" disabled={saving} onclick={addToHosts}>Add it</button>
      </p>
    {/if}
  </div>
</section>

<style>
  .address {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    padding: var(--space-7) var(--space-8);
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
  }
  .top {
    display: flex;
    align-items: center;
    gap: var(--space-5);
    min-width: 0;
  }
  .eyebrow {
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .url {
    font-size: 15px;
    font-weight: 600;
    color: var(--accent-fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    user-select: text;
  }
  .url.placeholder {
    color: var(--text-soft);
  }
  .url.broken {
    color: var(--error-text);
  }
  .spacer {
    flex: 1;
  }
  .icon-btn {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border: 1px solid var(--line);
    border-radius: var(--radius-button-sm);
    background: var(--bg-raised);
    color: var(--icon);
    cursor: pointer;
  }
  .icon-btn:hover {
    color: var(--text);
    border-color: var(--line-strong);
  }
  .open {
    height: 32px;
    padding: 0 var(--space-7);
    border: 0;
    border-radius: var(--radius-button-sm);
    background: var(--accent);
    color: var(--accent-ink);
    font-weight: 600;
    cursor: pointer;
  }
  .open:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .fields {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-6) var(--space-9);
  }
  .field {
    display: flex;
    align-items: center;
    gap: var(--space-5);
  }
  .lbl {
    font-size: 13px;
    font-weight: 600;
  }
  .name-box {
    display: inline-flex;
    height: 34px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-button-sm);
    background: var(--bg-app);
    overflow: hidden;
  }
  .name-box:focus-within {
    border-color: var(--accent-fg);
  }
  .name-box.bad {
    border-color: var(--error-border);
  }
  .name-box input {
    width: 170px;
    padding: 0 var(--space-4);
    border: 0;
    outline: none;
    background: transparent;
    color: var(--text);
    font-size: 13.5px;
    text-align: left;
    direction: ltr;
    user-select: text;
  }
  .ending {
    display: flex;
    align-items: center;
    padding: 0 var(--space-2);
    border-left: 1px solid var(--line);
    background: var(--bg-raised);
  }
  .seg {
    display: inline-flex;
    padding: 2px;
    gap: 2px;
    background: var(--bg-app);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-button-sm);
  }
  .seg button {
    height: 28px;
    padding: 0 var(--space-6);
    border: 0;
    border-radius: var(--radius-badge);
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-size: 13px;
    cursor: pointer;
  }
  .seg button.on {
    background: var(--bg-raised);
    color: var(--text);
    box-shadow: 0 0 0 1px var(--line);
  }
  .notes p {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-muted);
  }
  .notes p + p {
    margin-top: var(--space-2);
  }
  .notes .bad {
    color: var(--error-text);
  }
  .notes .warn {
    color: var(--text-soft);
  }
  .link {
    border: 0;
    padding: 0;
    margin-left: var(--space-2);
    background: transparent;
    color: var(--accent-fg);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
</style>
