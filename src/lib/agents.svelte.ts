// UI state for agents, terminals, settings and shortcuts (M6).

import { app } from "./app.svelte";
import * as api from "./api";
import type { AgentId, AgentsOverview, ProjectId, Settings, TerminalKind } from "./types";

class AgentsUi {
  overview = $state<AgentsOverview | null>(null);
  /** Top-level folders per project, for the working-folder select. */
  folders = $state<Record<ProjectId, string[]>>({});
  /** Per-project choice of terminal and folder in the Agents tab. */
  choice = $state<Record<ProjectId, { terminal: TerminalKind; subdir: string }>>({});
  checking = $state(false);
  launching = $state<string | null>(null);
  /** Short confirmation after a shortcut or launch. */
  toast = $state<{ text: string; tone: "ok" | "bad" } | null>(null);
  private toastTimer: ReturnType<typeof setTimeout> | null = null;

  async load() {
    try {
      this.overview = await api.agentsOverview();
    } catch (err) {
      app.error = api.toErrorDto(err);
    }
  }

  async recheck() {
    this.checking = true;
    try {
      app.tools = await api.toolsRecheck();
      await this.load();
    } finally {
      this.checking = false;
    }
  }

  /** Creates the project's terminal/folder choice from the defaults (call from an effect). */
  ensureChoice(projectId: ProjectId) {
    if (!this.choice[projectId] && this.overview) {
      this.choice[projectId] = { terminal: this.overview.settings.terminal, subdir: "" };
    }
  }

  async loadFolders(projectId: ProjectId) {
    this.folders[projectId] = await api.projectFolders(projectId).catch(() => []);
  }

  async launch(projectId: ProjectId, agent: AgentId | null, terminal: TerminalKind, subdir: string) {
    const key = `${projectId}:${agent ?? "terminal"}`;
    this.launching = key;
    try {
      await api.launchAgent(projectId, agent, terminal, subdir || null);
      app.error = null;
      const what = agent ? (this.overview?.agents.find((a) => a.id === agent)?.name ?? agent) : "Terminal";
      const where = this.overview?.terminals.find((t) => t.kind === terminal)?.name ?? terminal;
      this.showToast(`Opened ${what} in ${where}`, "ok");
      await app.reload(); // launch history and "recently opened"
    } catch (err) {
      app.error = api.toErrorDto(err);
      await this.load(); // the failure may have triggered a re-check
    } finally {
      this.launching = null;
    }
  }

  async setSettings(patch: Partial<Settings>) {
    if (!this.overview) return;
    const next = { ...this.overview.settings, ...patch };
    try {
      await api.setSettings(next);
      app.error = null;
    } catch (err) {
      app.error = api.toErrorDto(err);
    }
    await this.load();
  }

  async bind(slot: number, projectId: ProjectId, agent: AgentId | null, terminal: TerminalKind, subdir: string) {
    try {
      await api.setShortcut({ slot, projectId, agent, terminal, subdir: subdir || null });
      app.error = null;
    } catch (err) {
      app.error = api.toErrorDto(err);
    }
    await this.load();
  }

  async unbind(slot: number) {
    await api.removeShortcut(slot).catch((err) => (app.error = api.toErrorDto(err)));
    await this.load();
  }

  /** Ctrl+Alt+1..9 while DevDock has focus (T6.4). */
  async runShortcut(slot: number) {
    try {
      const what = await api.launchShortcut(slot);
      this.showToast(`Ctrl+Alt+${slot}: ${what}`, "ok");
      await app.reload();
    } catch (err) {
      const e = api.toErrorDto(err);
      this.showToast(e.message, "bad");
    }
  }

  showToast(text: string, tone: "ok" | "bad") {
    this.toast = { text, tone };
    if (this.toastTimer) clearTimeout(this.toastTimer);
    this.toastTimer = setTimeout(() => (this.toast = null), 3500);
  }
}

export const agents = new AgentsUi();
