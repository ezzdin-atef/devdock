// UI state shared across the shell. Domain data is a snapshot of Rust state:
// every change goes through a command, then the list is fetched again.

import * as api from "./api";
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
  query = $state<Query>({ search: "", library: "all", kind: null, sort: "recent" });
  lib = $state<LibraryView | null>(null);
  selectedId = $state<ProjectId | null>(null);
  justAddedId = $state<ProjectId | null>(null);
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
    await this.run(async () => {
      await api.refreshProjects();
      const [notice, tools] = await Promise.all([api.startupNotice(), api.toolsScan()]);
      this.notice = notice;
      this.tools = tools;
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
    await this.loadGitDetails(id, false);
    this.refreshGit();
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
    this.setQuery({ search: "", library: "all", kind: null });
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
      this.justAddedId = added.id;
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
