<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "$lib/app.svelte";
  import Rail from "$lib/chrome/Rail.svelte";
  import StatusStrip from "$lib/chrome/StatusStrip.svelte";
  import TitleBar from "$lib/chrome/TitleBar.svelte";
  import CloseDialog from "$lib/components/CloseDialog.svelte";
  import EndProcessDialog from "$lib/components/EndProcessDialog.svelte";
  import { ports } from "$lib/ports.svelte";
  import Ports from "$lib/screens/Ports.svelte";
  import { runs } from "$lib/runs.svelte";
  import { security } from "$lib/security.svelte";
  import Spaces from "$lib/screens/Spaces.svelte";
  import Running from "$lib/screens/Running.svelte";
  import Agents from "$lib/screens/Agents.svelte";
  import Settings from "$lib/screens/Settings.svelte";
  import Toast from "$lib/components/Toast.svelte";
  import Palette from "$lib/components/Palette.svelte";
  import { palette } from "$lib/palette.svelte";
  import { keyboard } from "$lib/keyboard";
  import { nav } from "$lib/nav.svelte";
  import { agents } from "$lib/agents.svelte";
  import { spaces } from "$lib/spaces.svelte";
  import Projects from "$lib/screens/Projects.svelte";
  import { theme } from "$lib/theme.svelte";
  import { updater } from "$lib/updater.svelte";
  import UpdateDialog from "$lib/components/UpdateDialog.svelte";

  onMount(() => {
    theme.init();
    updater.init();
    app.init();
    nav.init();
    runs.init();
    security.init();
    agents.load();
    spaces.init();

    // Ctrl+Alt+1..9 launches a bound agent/terminal (T6.4). Uses the physical key
    // (e.code) so it works on any keyboard layout; only while DevDock is focused.
    const onKey = (e: KeyboardEvent) => {
      const m = /^(?:Digit|Numpad)([1-9])$/.exec(e.code);
      if (m && e.ctrlKey && e.altKey && !e.shiftKey && !e.metaKey) {
        e.preventDefault();
        agents.runShortcut(Number(m[1]));
        return;
      }
      keyboard(e);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<div class="shell">
  <TitleBar />
  <div class="body">
    <Rail bind:current={app.view} />
    <main>
      {#if app.view === "projects"}
        <Projects />
      {:else if app.view === "spaces"}
        <Spaces />
      {:else if app.view === "running"}
        <Running />
      {:else if app.view === "ports"}
        <Ports />
      {:else if app.view === "agents"}
        <Agents />
      {:else}
        <Settings />
      {/if}
    </main>
  </div>
  <StatusStrip projectCount={app.lib?.counts.all ?? 0} tools={app.tools} />
</div>

{#if runs.closeRequested !== null}
  <CloseDialog />
{/if}

<Toast />

{#if palette.open}
  <Palette />
{/if}

{#if updater.open}
  <UpdateDialog />
{/if}

{#if ports.inspecting}
  <EndProcessDialog />
{/if}

<style>
  .shell {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  .body {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  main {
    flex: 1;
    min-width: 0;
    background: var(--bg-app);
  }
</style>
