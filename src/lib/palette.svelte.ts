// Command palette state (T8.2). Matching happens in Rust; this keeps the query,
// the results and which row is active, and carries out the chosen action.

import { agents } from "./agents.svelte";
import { app } from "./app.svelte";
import * as api from "./api";
import { ports } from "./ports.svelte";
import { runs } from "./runs.svelte";
import { spaces } from "./spaces.svelte";
import type { PaletteAction, PaletteGroup, PaletteHit } from "./types";

class PaletteUi {
  open = $state(false);
  query = $state("");
  groups = $state<PaletteGroup[]>([]);
  active = $state(0);
  loading = $state(false);

  /** Every hit in display order, for arrow-key movement. */
  flat = $derived<PaletteHit[]>(this.groups.flatMap((g) => g.hits));

  private seq = 0;

  async show() {
    if (this.open) return;
    this.open = true;
    this.query = "";
    this.active = 0;
    this.loading = true;
    try {
      this.groups = await api.paletteOpen();
    } catch (err) {
      app.error = api.toErrorDto(err);
      this.open = false;
    } finally {
      this.loading = false;
    }
  }

  close() {
    this.open = false;
  }

  toggle() {
    if (this.open) this.close();
    else this.show();
  }

  async setQuery(q: string) {
    this.query = q;
    const seq = ++this.seq;
    const groups = await api.paletteSearch(q).catch(() => this.groups);
    if (seq !== this.seq) return; // a newer keystroke won
    this.groups = groups;
    this.active = 0;
  }

  move(delta: number) {
    const n = this.flat.length;
    if (n) this.active = (this.active + delta + n) % n;
  }

  runActive() {
    const hit = this.flat[this.active];
    if (hit) this.run(hit.action);
  }

  async run(action: PaletteAction) {
    this.close();
    switch (action.kind) {
      case "openProject":
        await this.openProject(action.projectId, "overview");
        break;
      case "runCommand":
        await this.openProject(action.projectId, "run");
        await runs.start(action.projectId, action.commandId);
        break;
      case "launchAgent":
        await agents.launch(action.projectId, action.agent, agents.overview?.settings.terminal ?? "windowsTerminal", "");
        break;
      case "startWorkspace":
        app.view = "spaces";
        spaces.selectedId = action.workspaceId;
        await spaces.start(action.workspaceId);
        break;
      case "stopWorkspace":
        app.view = "spaces";
        spaces.selectedId = action.workspaceId;
        await spaces.stop(action.workspaceId);
        break;
      case "showPort":
        ports.filter = "all";
        ports.highlight = action.port;
        app.view = "ports";
        break;
      case "goTo":
        app.view = action.view;
        break;
      case "rescan":
        app.view = "projects";
        await app.rescan();
        break;
      case "stopEverything":
        app.view = "running";
        await runs.stopAll();
        break;
    }
  }

  private async openProject(id: string, tab: "overview" | "run") {
    app.view = "projects";
    app.clearFilters();
    app.tab = tab;
    await app.select(id);
  }
}

export const palette = new PaletteUi();
