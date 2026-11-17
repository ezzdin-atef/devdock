// UI state for workspaces (M7). Rust owns the orchestrators; this keeps snapshots
// and refetches when a `workspace:changed` event arrives.

import { agents } from "./agents.svelte";
import { app } from "./app.svelte";
import * as api from "./api";
import { runs } from "./runs.svelte";
import type { AfterStart, RestartPolicy, Service, SvcState, WorkspaceId, WorkspaceView } from "./types";

/** Colours for service labels in combined logs (DESIGN.md: blue, orange, text, ...). */
export const SERVICE_COLORS = ["var(--info)", "var(--svc-orange)", "var(--text)", "var(--success)", "var(--accent-fg)", "var(--svc-violet)"];

/** Bar colour class for the health bar in the list (DESIGN.md "Workspace rows"). */
export function tone(state: SvcState | "idle"): "ok" | "busy" | "bad" | "idle" {
  if (state === "healthy" || state === "up") return "ok";
  if (state === "starting" || state === "stopping") return "busy";
  if (state === "unhealthy" || state === "failed" || state === "gaveUp") return "bad";
  return "idle";
}

/** Services in display order (start order without dependencies). */
export function ordered(ws: WorkspaceView): Service[] {
  return [...ws.services].sort((a, b) => a.order - b.order || a.name.localeCompare(b.name));
}

class SpacesUi {
  list = $state<WorkspaceView[]>([]);
  selectedId = $state<WorkspaceId | null>(null);
  loaded = $state(false);
  selected = $derived(this.list.find((w) => w.id === this.selectedId) ?? null);

  private timer: ReturnType<typeof setTimeout> | null = null;
  private started = false;

  init() {
    if (this.started) return;
    this.started = true;
    this.load();
    api.onWorkspaceChanged(() => this.scheduleLoad());
    api.onPresetError((e) => agents.showToast(e.message, "bad"));
  }

  scheduleLoad() {
    if (this.timer) return;
    this.timer = setTimeout(() => {
      this.timer = null;
      this.load();
      runs.refreshList();
    }, 100);
  }

  async load() {
    try {
      this.list = await api.workspacesList();
      this.loaded = true;
      if (!this.selectedId && this.list.length) this.selectedId = this.list[0].id;
    } catch (err) {
      app.error = api.toErrorDto(err);
    }
  }

  /** Runs an action, reports its error, reloads. Returns true on success. */
  private async act(call: () => Promise<unknown>): Promise<boolean> {
    try {
      await call();
      app.error = null;
      return true;
    } catch (err) {
      app.error = api.toErrorDto(err);
      return false;
    } finally {
      await this.load();
    }
  }

  async create(name: string) {
    let id: WorkspaceId | null = null;
    const ok = await this.act(async () => (id = await api.workspaceCreate(name)));
    if (ok && id) this.selectedId = id;
    return ok;
  }

  rename = (id: WorkspaceId, name: string) => this.act(() => api.workspaceRename(id, name));
  remove = async (id: WorkspaceId) => {
    const ok = await this.act(() => api.workspaceDelete(id));
    if (ok && this.selectedId === id) this.selectedId = this.list[0]?.id ?? null;
  };
  upsert = (id: WorkspaceId, svc: Service) => this.act(() => api.workspaceUpsertService(id, svc));
  removeService = (id: WorkspaceId, serviceId: string) =>
    this.act(() => api.workspaceRemoveService(id, serviceId));
  setRules = (id: WorkspaceId, restart: RestartPolicy, after: AfterStart) =>
    this.act(() => api.workspaceSetRules(id, restart, after));
  start = (id: WorkspaceId) => this.act(() => api.workspaceStart(id));
  stop = (id: WorkspaceId) => this.act(() => api.workspaceStop(id));
}

export const spaces = new SpacesUi();
