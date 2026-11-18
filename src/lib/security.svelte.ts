// Security tab state. Rust runs the scanners and saves each report; this keeps the
// latest report per project (loaded on demand), the scans in flight, and a summary
// per project for the badges on the Projects page.

import * as api from "./api";
import type {
  ErrorDto,
  InstallEvent,
  InstallLog,
  ProjectId,
  ScannerInfo,
  SecurityProgress,
  SecurityReport,
  SecuritySummary,
  Severity,
} from "./types";

export const SEVERITIES: Severity[] = ["critical", "high", "medium", "low", "unknown"];
export const SEVERITY_LABEL: Record<Severity, string> = {
  critical: "Critical",
  high: "High",
  medium: "Medium",
  low: "Low",
  unknown: "Unknown",
};

interface Active {
  startedAt: number;
  progress: SecurityProgress | null;
}

class SecurityUi {
  reports = $state<Record<ProjectId, SecurityReport | null>>({});
  loading = $state<Record<ProjectId, true>>({});
  running = $state<Record<ProjectId, Active>>({});
  summaries = $state<Record<ProjectId, SecuritySummary>>({});
  scanners = $state<ScannerInfo[] | null>(null);
  errors = $state<Record<ProjectId, ErrorDto | null>>({});
  /** "Include git history" checkbox, per project for this session. */
  history = $state<Record<ProjectId, boolean>>({});
  /** A cancel was asked for and the scan hasn't stopped yet. */
  stopping = $state<Record<ProjectId, true>>({});
  /** Scanner installs in flight: the stage each is at. */
  installing = $state<Record<string, string>>({});
  /** The last install outcome per scanner, shown under its row. */
  installNote = $state<Record<string, { tone: "ok" | "bad"; text: string }>>({});
  /** Download progress per scanner (0 to 1), null when the step can't tell. */
  installProgress = $state<Record<string, number | null>>({});
  /** The newest install log per scanner this session, running or finished. */
  installLogs = $state<Record<string, InstallLog>>({});
  /** The scanner whose install log is open in the dialog. */
  logOpen = $state<string | null>(null);

  private onInstall(e: InstallEvent) {
    let log = this.installLogs[e.tool];
    if (!log || (e.state === "running" && log.state !== "running")) {
      this.installLogs[e.tool] = {
        tool: e.tool,
        name: this.scanners?.find((s) => s.id === e.tool)?.name ?? e.tool,
        state: "running",
        stage: e.stage,
        progress: null,
        lines: [],
        dropped: 0,
        error: null,
        startedAt: Date.now(),
        finishedAt: null,
      };
      // Read it back through the store, so changes below are tracked.
      log = this.installLogs[e.tool];
    }
    if (e.line !== null) log.lines.push(e.line);
    log.state = e.state;
    log.stage = e.stage;
    log.progress = e.progress;
    log.error = e.error;
    if (e.state !== "running") log.finishedAt = Date.now();
  }

  async openLog(tool: string) {
    this.logOpen = tool;
    try {
      const full = await api.securityInstallLog(tool);
      if (full) this.installLogs[tool] = full;
    } catch {
      // Keep what the events gave us.
    }
  }

  init() {
    api.onSecurityProgress((p) => {
      this.running[p.projectId] = { startedAt: this.running[p.projectId]?.startedAt ?? p.startedAt, progress: p };
    });
    api.onSecurityDone((d) => {
      delete this.running[d.projectId];
      delete this.stopping[d.projectId];
      if (d.summary) this.summaries[d.projectId] = d.summary;
      this.errors[d.projectId] = d.error;
      if (d.projectId in this.reports) this.load(d.projectId);
    });
    api.onSecurityInstall((e) => {
      this.onInstall(e);
      if (e.state === "running") {
        this.installing[e.tool] = e.stage;
        this.installProgress[e.tool] = e.progress;
        return;
      }
      delete this.installing[e.tool];
      delete this.installProgress[e.tool];
      if (e.state === "done") this.installNote[e.tool] = { tone: "ok", text: "Installed." };
      else if (e.state === "failed") this.installNote[e.tool] = { tone: "bad", text: e.error ?? "The install failed." };
      else delete this.installNote[e.tool];
      this.loadScanners(true);
    });
    api
      .securityInstalling()
      .then((list) => {
        for (const e of list) {
          this.installing[e.tool] = e.stage;
          this.installProgress[e.tool] = e.progress;
        }
      })
      .catch(() => {});
    api
      .securityRunning()
      .then((list) => {
        for (const r of list) this.running[r.projectId] = { startedAt: r.startedAt, progress: r.progress };
      })
      .catch(() => {});
    api
      .securitySummaries()
      .then((s) => (this.summaries = s))
      .catch(() => {});
  }

  async load(id: ProjectId) {
    this.loading[id] = true;
    try {
      this.reports[id] = await api.securityReport(id);
    } catch (err) {
      this.errors[id] = api.toErrorDto(err);
    } finally {
      delete this.loading[id];
    }
  }

  async loadScanners(force = false) {
    if (this.scanners && !force) return;
    try {
      this.scanners = await api.securityScanners();
    } catch {
      this.scanners = [];
    }
  }

  async install(tool: string) {
    delete this.installNote[tool];
    this.installing[tool] = "Starting";
    try {
      await api.securityInstall(tool);
    } catch (err) {
      delete this.installing[tool];
      this.installNote[tool] = { tone: "bad", text: api.toErrorDto(err).message };
    }
  }

  cancelInstall(tool: string) {
    api.securityInstallCancel(tool).catch(() => {});
  }

  async uninstall(tool: string) {
    try {
      await api.securityUninstall(tool);
      delete this.installNote[tool];
    } catch (err) {
      this.installNote[tool] = { tone: "bad", text: api.toErrorDto(err).message };
    }
    this.loadScanners(true);
  }

  async scan(id: ProjectId) {
    this.errors[id] = null;
    this.running[id] = { startedAt: Date.now(), progress: null };
    try {
      await api.securityScan(id, { history: this.history[id] ?? false });
    } catch (err) {
      delete this.running[id];
      this.errors[id] = api.toErrorDto(err);
    }
  }

  async cancel(id: ProjectId) {
    this.stopping[id] = true;
    await api.securityCancel(id).catch(() => {});
  }

  /** Saves the latest report as HTML and opens it. Returns the path, or null if cancelled. */
  async export(id: ProjectId, name: string): Promise<string | null> {
    const day = new Date().toISOString().slice(0, 10);
    const safe = name.replace(/[^\w.-]+/g, "-");
    const path = await api.pickReportFile(`${safe}-security-${day}.html`);
    if (!path) return null;
    try {
      await api.securityExport(id, path);
      await api.securityOpenFile(path).catch(() => {});
      return path;
    } catch (err) {
      this.errors[id] = api.toErrorDto(err);
      return null;
    }
  }

  /** The project-list badge: worst thing found, or null when nothing worth a badge. */
  badge(id: ProjectId): { text: string; tone: "bad" | "warn" } | null {
    const s = this.summaries[id];
    if (!s) return null;
    const c = s.counts;
    if (c.secrets) return { text: c.secrets === 1 ? "1 secret" : `${c.secrets} secrets`, tone: "bad" };
    if (c.critical) return { text: `${c.critical} critical`, tone: "bad" };
    if (c.high) return { text: `${c.high} high`, tone: "warn" };
    return null;
  }
}

export const security = new SecurityUi();
