<script lang="ts">
  import Checkbox from "$lib/components/Checkbox.svelte";
  import { agents } from "$lib/agents.svelte";
  import { app } from "$lib/app.svelte";
  import * as api from "$lib/api";
  import Button from "$lib/components/Button.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import type { ProxyStatus } from "$lib/types";

  // Local addresses: proxy (no port), HTTPS (local certificate), *.test (hosts file).
  const settings = $derived(agents.overview?.settings);
  let status = $state<ProxyStatus | null>(null);
  let busy = $state<string | null>(null);
  let ports = $state({ http: "", https: "" });

  async function load() {
    try {
      status = await api.proxyStatus();
    } catch (err) {
      app.error = api.toErrorDto(err);
    }
  }
  $effect(() => {
    void settings?.proxy;
    void settings?.https;
    void settings?.proxyPort;
    void settings?.httpsPort;
    load();
  });
  $effect(() => {
    ports = { http: String(settings?.proxyPort ?? 80), https: String(settings?.httpsPort ?? 443) };
  });

  async function act(name: string, call: () => Promise<void>) {
    busy = name;
    try {
      await call();
      app.error = null;
    } catch (err) {
      app.error = api.toErrorDto(err);
    } finally {
      busy = null;
      await load();
    }
  }

  function savePort(which: "http" | "https") {
    const n = Number(ports[which]);
    if (!Number.isInteger(n) || n < 1 || n > 65535) return;
    agents.setSettings(which === "http" ? { proxyPort: n } : { httpsPort: n });
  }

  const sample = $derived.by(() => {
    const p = settings?.proxyPort ?? 80;
    return `http://shop.localhost${p === 80 ? "" : `:${p}`}`;
  });
</script>

<section class="panel">
  <div class="head">
    <span class="title">Local addresses</span>
    <span class="spacer"></span>
    <Button variant="ghost" onclick={load} title="Check again"><Icon name="refresh" size={13} /></Button>
  </div>

  <label class="row toggle">
    <Checkbox variant="switch" checked={settings?.proxy ?? false} onchange={(v) => agents.setSettings({ proxy: v })} />
    <span class="grow">
      Hide ports for all projects
      <span class="help block"
        >Open projects at <span class="mono">{sample}</span> instead of adding <span class="mono">:3000</span>. Each
        project can change this under Address.</span
      >
      {#if status?.httpError}
        <span class="bad block">{status.httpError}</span>
      {:else if status?.http}
        <span class="ok block"
          >Listening on port {status.http}{settings?.proxy ? "" : " for projects that hide their port"}.</span
        >
        {#if status.httpNote}<span class="help block">{status.httpNote}</span>{/if}
      {/if}
    </span>
    <span class="port">
      port
      <input
        class="mono"
        inputmode="numeric"
        bind:value={ports.http}
        onchange={() => savePort("http")}
        aria-label="Proxy port"
      />
    </span>
  </label>

  <label class="row toggle">
    <Checkbox variant="switch" checked={settings?.https ?? false} onchange={(v) => agents.setSettings({ https: v })} />
    <span class="grow">
      Also serve HTTPS
      <span class="help block"
        >For projects that hide their port. For secure cookies, service workers and OAuth callbacks. DevDock signs a certificate for each address with
        its own local authority, which only covers .localhost and .test names.</span
      >
      {#if settings?.https}
        {#if status?.httpsError}
          <span class="bad block">{status.httpsError}</span>
        {:else if status?.https}
          <span class="ok block">Listening on port {status.https}.</span>
          {#if status.httpsNote}<span class="help block">{status.httpsNote}</span>{/if}
        {/if}
      {/if}
    </span>
    <span class="port">
      port
      <input
        class="mono"
        inputmode="numeric"
        bind:value={ports.https}
        onchange={() => savePort("https")}
        aria-label="HTTPS port"
      />
    </span>
  </label>

  {#if status?.ca}
    <div class="row">
      <span class="grow">
        DevDock certificate
        {#if status.ca.trusted}<span class="ok">trusted</span>{:else}<span class="bad">not trusted yet</span>{/if}
        <span class="help block"
          >Browsers warn on HTTPS addresses until you trust it. Windows asks you to confirm. Firefox uses its own list:
          import <span class="mono selectable">{status.ca.path}</span> there by hand.</span
        >
      </span>
      {#if status.ca.trusted}
        <Button variant="secondary" disabled={busy !== null} onclick={() => act("untrust", api.caUntrust)}>
          Remove
        </Button>
      {:else}
        <Button variant="primary" disabled={busy !== null} onclick={() => act("trust", api.caTrust)}>
          {busy === "trust" ? "Waiting for Windows…" : "Trust"}
        </Button>
      {/if}
    </div>
  {/if}

  <div class="row">
    <span class="grow">
      .test addresses
      {#if status?.hosts.needed.length}
        <span class="help block">
          {#if status.hosts.missing.length}
            Not in the hosts file yet: <span class="mono">{status.hosts.missing.join(", ")}</span>.
          {:else}
            All in the hosts file: <span class="mono">{status.hosts.needed.join(", ")}</span>.
          {/if}
        </span>
      {:else}
        <span class="help block"
          >Name a project <span class="mono">shop.test</span> (its Overview, under Address) to use .test. Those need a hosts-file entry;
          .localhost names work without one.</span
        >
      {/if}
      {#if status?.hosts.error}<span class="bad block">{status.hosts.error}</span>{/if}
    </span>
    {#if status && (status.hosts.missing.length || status.hosts.stale)}
      <Button variant="secondary" disabled={busy !== null} onclick={() => act("hosts", api.hostsSync)}>
        {busy === "hosts" ? "Waiting for Windows…" : "Update hosts file"}
      </Button>
    {/if}
  </div>
</section>

<style>
  .panel {
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-4) var(--space-6) var(--space-4) var(--space-8);
    border-bottom: 1px solid var(--line);
  }
  .title {
    font-weight: 600;
  }
  .spacer,
  .grow {
    flex: 1;
    min-width: 0;
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
  .toggle {
    align-items: flex-start;
    cursor: pointer;
  }
  .help {
    font-size: 12.5px;
    color: var(--text-muted);
  }
  .block {
    display: block;
    margin-top: var(--space-1);
  }
  .ok {
    font-size: 12.5px;
    color: var(--success);
  }
  .bad {
    font-size: 12.5px;
    color: var(--error-text);
  }
  .port {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    font-size: 12.5px;
    color: var(--text-muted);
  }
  .port input {
    width: 64px;
    height: 30px;
    padding: 0 var(--space-4);
    background: var(--bg-app);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-button-sm);
    color: var(--text);
    font-size: 13px;
    user-select: text;
  }
  .port input:focus {
    outline: none;
    border-color: var(--accent-fg);
  }
  .selectable {
    user-select: text;
    overflow-wrap: anywhere;
  }
</style>
