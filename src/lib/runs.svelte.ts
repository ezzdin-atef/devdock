// UI state for running commands (M4). Rust's Runner owns the processes and logs;
// this keeps snapshots and refetches when a `run:changed` event says something moved.

import { app } from "./app.svelte";
import * as api from "./api";
import {
  isActive,
  type CommandId,
  type CommandList,
  type Conflict,
  type ProjectId,
  type RunChanged,
  type RunInfo,
} from "./types";

export const runKey = (projectId: ProjectId, commandId: CommandId) => `${projectId}::${commandId}`;

class RunsUi {
  /** Every run this session (Running screen, title-bar count, card states). */
  list = $state<RunInfo[]>([]);
  /** Run-tab command lists, per project, loaded on demand. */
  commandsByProject = $state<Record<ProjectId, CommandList>>({});
  /** The command whose console is shown in each project's Run tab. */
  selectedCommand = $state<Record<ProjectId, CommandId>>({});
  /** Bumped per run key when new log lines exist; consoles watch their key. */
  logTick = $state<Record<string, number>>({});
  /** Set when the window close was blocked because processes are running. */
  closeRequested = $state<number | null>(null);
  /** Ticks every second so uptimes move. */
  now = $state(Date.now());
  /** Start/stop/restart requests in flight, per run key. */
  busy = $state<Record<string, true>>({});
  /** A start that was held back because its port is taken, per run key (T5.5). */
  conflicts = $state<Record<string, Conflict>>({});

  active = $derived(this.list.filter((r) => isActive(r.state)));

  private refreshTimer: ReturnType<typeof setTimeout> | null = null;
  private dirtyProjects = new Set<ProjectId>();

  init() {
    this.refreshList();
    api.onRunChanged((c) => this.onChanged(c));
    api.onCloseRequested((n) => (this.closeRequested = n));
    setInterval(() => (this.now = Date.now()), 1000);
  }

  private onChanged(c: RunChanged) {
    if (c.logs) {
      const k = runKey(c.projectId, c.commandId);
      this.logTick[k] = (this.logTick[k] ?? 0) + 1;
    }
    if (c.state) {
      this.dirtyProjects.add(c.projectId);
      if (!this.refreshTimer) {
        this.refreshTimer = setTimeout(() => this.flush(), 60);
      }
    }
  }

  private async flush() {
    this.refreshTimer = null;
    const projects = [...this.dirtyProjects];
    this.dirtyProjects.clear();
    await this.refreshList();
    for (const p of projects) if (this.commandsByProject[p]) await this.loadCommands(p);
  }

  async refreshList() {
    this.list = await api.runsList().catch(() => this.list);
  }

  async loadCommands(projectId: ProjectId) {
    try {
      const list = await api.runCommands(projectId);
      this.commandsByProject[projectId] = list;
      const chosen = this.selectedCommand[projectId];
      if (!chosen || !list.commands.some((c) => c.id === chosen)) {
        const running = list.commands.find((c) => isActive(c.run?.state));
        const first = running ?? list.commands.find((c) => c.run) ?? list.commands[0];
        if (first) this.selectedCommand[projectId] = first.id;
      }
    } catch (err) {
      app.error = api.toErrorDto(err);
    }
  }

  runOf(projectId: ProjectId, commandId: CommandId): RunInfo | undefined {
    return this.list.find((r) => r.projectId === projectId && r.commandId === commandId);
  }

  activeIn(projectId: ProjectId): number {
    return this.active.filter((r) => r.projectId === projectId).length;
  }

  private async act(projectId: ProjectId, commandId: CommandId, call: () => Promise<void>) {
    const k = runKey(projectId, commandId);
    this.busy[k] = true;
    try {
      await call();
      app.error = null;
    } catch (err) {
      app.error = api.toErrorDto(err);
    } finally {
      delete this.busy[k];
      await this.refreshList();
      if (this.commandsByProject[projectId]) await this.loadCommands(projectId);
    }
  }

  /** Starts a command. If its known port is taken, records a conflict instead. */
  start(projectId: ProjectId, commandId: CommandId, opts: { force?: boolean; port?: number } = {}) {
    this.selectedCommand[projectId] = commandId;
    const k = runKey(projectId, commandId);
    return this.act(projectId, commandId, async () => {
      const out = await api.runStart(projectId, commandId, opts);
      if (out.outcome === "conflict") this.conflicts[k] = out.conflict;
      else delete this.conflicts[k];
    });
  }

  /** Resolves once a run is no longer active (or after `ms`). */
  async waitEnded(projectId: ProjectId, commandId: CommandId, ms = 10000) {
    const until = Date.now() + ms;
    while (Date.now() < until) {
      await this.refreshList();
      if (!isActive(this.runOf(projectId, commandId)?.state)) return;
      await new Promise((r) => setTimeout(r, 150));
    }
  }

  dismissConflict(projectId: ProjectId, commandId: CommandId) {
    delete this.conflicts[runKey(projectId, commandId)];
  }

  stop(projectId: ProjectId, commandId: CommandId) {
    return this.act(projectId, commandId, () => api.runStop(projectId, commandId));
  }

  restart(projectId: ProjectId, commandId: CommandId) {
    return this.act(projectId, commandId, () => api.runRestart(projectId, commandId));
  }

  async stopAll() {
    try {
      await api.runStopAll();
    } catch (err) {
      app.error = api.toErrorDto(err);
    }
    await this.refreshList();
  }

  async addCommand(projectId: ProjectId, name: string, line: string, subdir: string): Promise<boolean> {
    try {
      const id = await api.addCommand(projectId, name, line, subdir.trim() || null);
      app.error = null;
      this.selectedCommand[projectId] = id;
      await this.loadCommands(projectId);
      return true;
    } catch (err) {
      app.error = api.toErrorDto(err);
      return false;
    }
  }

  async removeCommand(projectId: ProjectId, commandId: CommandId) {
    try {
      await api.removeCommand(projectId, commandId);
      app.error = null;
    } catch (err) {
      app.error = api.toErrorDto(err);
    }
    await this.loadCommands(projectId);
  }

  /** From the Running screen: jump to a run's console in its project. */
  async showLogs(projectId: ProjectId, commandId: CommandId) {
    app.view = "projects";
    app.clearFilters();
    app.tab = "run";
    this.selectedCommand[projectId] = commandId;
    await app.select(projectId);
  }

  async quit() {
    this.closeRequested = null;
    await api.quitApp().catch((err) => (app.error = api.toErrorDto(err)));
  }
}

export const runs = new RunsUi();
