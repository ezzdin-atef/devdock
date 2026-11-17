// UI state shared across the shell. Domain data is a snapshot of Rust state:
// every change goes through a command, then the list is fetched again.

import { agents } from "./agents.svelte";
import * as api from "./api";
import { view } from "./viewPrefs.svelte";
import type {
  DetailTab,
  ErrorDto,
  LibraryView,
  ProjectId,
  Query,
  RepoInfo,
  Snapshot,
  ScanProgress,
  ScanSummary,
  ToolScan,
  View,
} from "./types";

class AppUi {
  view = $state<View>("projects");
  query = $state<Query>({ search: "", library: "all", kind: null, sort: view.sort, group: null });
  /** A group whose name is being edited in place (just created, or "Rename"). */
  renamingGroup = $state<string | null>(null);
  lib = $state<LibraryView | null>(null);
  selectedId = $state<ProjectId | null>(null);
  renamingId = $state<ProjectId | null>(null);
  tools = $state<ToolScan | null>(null);
  error = $state<ErrorDto | null>(null);
  /** One-time startup notice, e.g. "restored from backup". */
  notice = $state<ErrorDto | null>(null);
  scanProgress = $state<ScanProgress | null>(null);
  scanSummary = $state<ScanSummary | null>(null);
  /** Kept when switching projects (DESIGN.md section 9). */
  tab = $state<DetailTab>("overview");
  /**
   * Git-tab facts and in-flight refreshes, keyed by project. Keying (instead of one
   * "current" slot) means a slow reply for one project can never show up under another.
   */
  private gitDetailsById = $state<Record<ProjectId, Snapshot<RepoInfo>>>({});
  private gitLoadingIds = $state<Record<ProjectId, true>>({});

  selected = $derived(this.lib?.projects.find((p) => p.id === this.selectedId) ?? null);
  scanning = $derived(this.lib?.scanning ?? false);
  gitDetails = $derived(this.selectedId ? (this.gitDetailsById[this.selectedId] ?? null) : null);
  gitLoading = $derived(this.selectedId ? this.selectedId in this.gitLoadingIds : false);

  private requestSeq = 0;
  private reloadTimer: ReturnType<typeof setTimeout> | null = null;

  async init() {
    // The saved list first, so names show at once; re-checking every folder and
    // finding tools can take seconds on a big library, so they follow.
    await this.reload();
    this.run(async () => {
      const [notice, tools] = await Promise.all([api.startupNotice(), api.toolsScan()]);
      this.notice = notice;
      this.tools = tools;
    });
    this.run(async () => {
      await api.refreshProjects();
      await this.reload();
    });
    api.gitRefresh([], false).catch(() => {});
    api.onGitUpdated(({ id, details }) => {
      this.scheduleReload();
      if (details) this.loadGitDetails(id, true);
    });
    api.onScanProgress((p) => {
      this.scanProgress = p;
      this.reload();
    });
    api.onScanDone((s) => {
      this.scanProgress = null;
      this.scanSummary = s;
      this.reload();
      if (s.added > 0) api.gitRefresh([], false).catch(() => {});
    });
    api.onStoreError((e) => (this.error = e));
    // Folders can vanish or come back while DevDock is in the background.
    window.addEventListener("focus", () => this.refreshOnFocus());
  }

  /** Fetches the list for the current query. Ignores replies to older requests. */
  async reload() {
    const seq = ++this.requestSeq;
    try {
      const lib = await api.library($state.snapshot(this.query));
      if (seq === this.requestSeq) this.lib = lib;
    } catch (err) {
      this.error = api.toErrorDto(err);
    }
  }

  /** Coalesces bursts of events (one per project) into one list fetch. */
  private scheduleReload() {
    if (this.reloadTimer) return;
    this.reloadTimer = setTimeout(() => {
      this.reloadTimer = null;
      this.reload();
    }, 120);
  }

  /** Selects a project and refreshes its Git facts in the background. */
  async select(id: ProjectId | null) {
    this.selectedId = id;
    if (id === null) return;
    this.markSeen(id);
    await this.loadGitDetails(id, false);
    this.refreshGit();
  }

  /** Selecting a project counts as seeing it: its "new from scan" dot goes away. */
  private markSeen(id: ProjectId) {
    const p = this.lib?.projects.find((x) => x.id === id);
    if (!p?.isNew) return;
    p.isNew = false; // hide the dot now; Rust confirms below
    if (this.lib && this.lib.counts.new > 0) this.lib.counts.new -= 1;
    api
      .markSeen(id)
      .then((cleared) => {
        // In the "New from scan" view, keep the row where it is until the next refresh.
        if (cleared && this.query.library !== "new") this.scheduleReload();
      })
      .catch(() => this.reload());
  }

  refreshGit() {
    const id = this.selectedId;
    if (id === null || this.selected?.missing) return;
    this.gitLoadingIds[id] = true;
    api.gitRefresh([id], true).catch((err) => {
      delete this.gitLoadingIds[id];
      this.error = api.toErrorDto(err);
    });
  }

  /** Copies Rust's cached details for `id`; `done` = a refresh for it just finished. */
  private async loadGitDetails(id: ProjectId, done: boolean) {
    const details = await api.gitDetails(id).catch(() => null);
    if (details) this.gitDetailsById[id] = details;
    if (done) delete this.gitLoadingIds[id];
  }

  setQuery(patch: Partial<Query>) {
    Object.assign(this.query, patch);
    this.reload();
  }

  clearFilters() {
    this.setQuery({ search: "", library: "all", kind: null, group: null });
  }

  /** Makes a group and starts renaming it in place; moves `projectIds` into it. */
  async createGroup(name = "New group", projectIds: ProjectId[] = []) {
    let unique = name;
    const taken = new Set((this.lib?.groups ?? []).map((g) => g.name.toLowerCase()));
    for (let n = 2; taken.has(unique.toLowerCase()); n++) unique = `${name} ${n}`;
    await this.run(async () => {
      const id = await api.groupCreate(unique);
      if (projectIds.length) await api.projectsSetGroup(projectIds, id);
      // Making a group means you want to see groups.
      if (view.grouping !== "groups") view.set({ grouping: "groups" });
      this.renamingGroup = id;
    });
    await this.reload();
  }

  async updateGroup(id: string, patch: { name?: string; color?: string }) {
    await this.run(() => api.groupUpdate(id, patch));
    await this.reload();
  }

  async moveGroup(id: string, delta: number) {
    await this.run(() => api.groupMove(id, delta));
    await this.reload();
  }

  async reorderGroup(id: string, index: number) {
    // Move it locally first so the list doesn't jump back while saving.
    const list = this.lib?.groups;
    const from = list?.findIndex((g) => g.id === id) ?? -1;
    if (list && from >= 0) {
      const [g] = list.splice(from, 1);
      list.splice(Math.min(index, list.length), 0, g);
    }
    await this.run(() => api.groupReorder(id, index));
    await this.reload();
  }

  async deleteGroup(id: string) {
    await this.run(() => api.groupDelete(id));
    if (this.query.group === id) this.query.group = null;
    await this.reload();
  }

  /**
   * Drag and drop between projects: before or after `anchor`, and into `group`
   * when `regroup`. Switches the page to the manual order so the result stays put.
   */
  async placeProjects(ids: ProjectId[], anchor: ProjectId, after: boolean, regroup: boolean, group: string | null) {
    const list = this.lib?.projects;
    if (list && !ids.includes(anchor)) {
      // Move them locally first so the cards slide into place right away.
      const moved = list.filter((p) => ids.includes(p.id));
      for (const p of moved) {
        list.splice(list.indexOf(p), 1);
        if (regroup) p.group = group;
      }
      const at = list.findIndex((p) => p.id === anchor) + (after ? 1 : 0);
      list.splice(at, 0, ...moved);
    }
    if (this.query.sort !== "manual") {
      view.set({ sort: "manual" });
      this.query.sort = "manual";
      agents.showToast("Sorted by your own order now", "ok");
    }
    await this.run(() => api.projectsPlace(ids, anchor, after, regroup, group));
    await this.reload();
  }

  /** Puts projects in a group (`null` = ungrouped). */
  async moveToGroup(ids: ProjectId[], group: string | null) {
    await this.run(() => api.projectsSetGroup(ids, group));
    await this.reload();
  }


  async refreshOnFocus() {
    await api.refreshProjects().catch(() => {});
    this.tools = await api.toolsScan().catch(() => this.tools);
    await this.reload();
    api.gitRefresh([], false).catch(() => {});
    this.refreshGit();
  }

  async addProject() {
    await this.run(async () => {
      const path = await api.pickFolder("Dock a project folder");
      if (path === null) return; // cancelled: do nothing
      const added = await api.addProject(path);
      // Make sure the new project is visible even if a filter would hide it.
      this.clearFilters();
      await this.reload();
      await this.select(added.id);
    });
  }

  async addScanRoot() {
    await this.run(async () => {
      const path = await api.pickFolder("Choose a folder to scan for projects");
      if (path === null) return;
      this.scanSummary = null;
      await api.addScanRoot(path);
    });
    await this.reload();
  }

  async removeScanRoot(path: string) {
    await this.mutate(() => api.removeScanRoot(path));
  }

  async rescan() {
    this.scanSummary = null;
    await this.mutate(() => api.rescan());
  }

  reviewNew() {
    this.setQuery({ library: "new", kind: null, search: "" });
  }

  dismissScanSummary() {
    this.scanSummary = null;
    if (this.query.library === "new") this.setQuery({ library: "all" });
  }

  async toggleFavorite(id: ProjectId, favorite: boolean) {
    await this.mutate(() => api.setFavorite(id, favorite));
  }

  startRename(id: ProjectId) {
    if (id !== this.selectedId) this.select(id);
    this.renamingId = id;
  }

  async finishRename(id: ProjectId, name: string | null) {
    this.renamingId = null;
    if (name === null) return; // Esc: keep the old name
    await this.mutate(() => api.renameProject(id, name));
  }

  async remove(id: ProjectId) {
    await this.mutate(async () => {
      await api.removeProject(id);
      if (this.selectedId === id) this.selectedId = null;
    });
  }

  async locate(id: ProjectId) {
    await this.mutate(async () => {
      const path = await api.pickFolder("Where is this project now?");
      if (path === null) return;
      const newId = await api.relocateProject(id, path);
      await this.reload();
      await this.select(newId);
    });
  }

  async openInExplorer(id: ProjectId) {
    await this.mutate(() => api.openInExplorer(id));
  }

  async openInEditor(id: ProjectId) {
    await this.mutate(() => api.openInEditor(id));
    // Re-check so the status strip reflects a VS Code install or removal.
    this.tools = await api.toolsScan().catch(() => this.tools);
  }

  async copyPath(path: string) {
    await this.run(() => navigator.clipboard.writeText(path));
  }

  /** Clears a project's recent activity and launch history. */
  async clearHistory(id: ProjectId) {
    await this.run(() => api.projectClearHistory(id));
    await this.reload();
  }

  /** Settings, Data: clears history everywhere (and learned ports when asked). */
  async clearAllHistory(learnedPorts: boolean) {
    await this.run(() => api.clearAllHistory(learnedPorts));
    await this.reload();
  }

  dismissError() {
    this.error = null;
  }

  /** Runs a command that changes state, then refreshes the list either way. */
  private async mutate(action: () => Promise<unknown>) {
    await this.run(action);
    await this.reload();
  }

  private async run(action: () => Promise<unknown>) {
    try {
      await action();
      this.error = null;
    } catch (err) {
      this.error = api.toErrorDto(err);
    }
  }
}

export const app = new AppUi();
