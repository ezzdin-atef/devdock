// Hand-written mirrors of the Rust DTOs (src-tauri/src). Keep in sync.

/** project.rs `ProjectId`: 16 hex digits (a u64 would lose precision in JS). */
export type ProjectId = string;

/** detect.rs `ProjectKind` */
export type Kind = "nextJs" | "node" | "rust" | "php" | "compose" | "unknown";

export const KIND_LABEL: Record<Kind, string> = {
  nextJs: "NEXT.JS",
  node: "NODE",
  rust: "RUST",
  php: "PHP",
  compose: "COMPOSE",
  unknown: "FOLDER",
};

/** git/status.rs `GitStatus` */
export interface GitStatus {
  oid: string | null;
  /** null when HEAD is detached */
  branch: string | null;
  upstream: string | null;
  ahead: number | null;
  behind: number | null;
  staged: number;
  modified: number;
  untracked: number;
  conflicted: number;
  changedFiles: number;
}

/** git/facts.rs */
export interface LastCommit {
  hash: string;
  subject: string;
  author: string;
  relative: string;
}
export interface Remote {
  name: string;
  url: string;
}
export interface Worktree {
  path: string;
  branch: string | null;
  detached: boolean;
  bare: boolean;
  isCurrent: boolean;
}
export interface RepoInfo {
  root: string;
  isWorktree: boolean;
  inSubfolder: boolean;
  status: GitStatus;
  lastCommit: LastCommit | null;
  remotes: Remote[];
  worktrees: Worktree[];
}

/** git/mod.rs `Snapshot<T>`: "not a repo" is a state, not an error. */
export type Snapshot<T> =
  | { state: "repo"; data: T; refreshedAt: number }
  | { state: "noRepo"; refreshedAt: number }
  | { state: "error"; error: ErrorDto; refreshedAt: number };

/** state.rs `ProjectView` (a `Project` plus live flags) */
export interface Project {
  id: ProjectId;
  name: string;
  path: string;
  kind: Kind;
  isGit: boolean;
  favorite: boolean;
  lastOpened: number | null;
  addedAt: number;
  missing: boolean;
  isNew: boolean;
  /** absent until the first git refresh finishes */
  git: Snapshot<GitStatus> | null;
  /** newest first */
  launches: LaunchRecord[];
  /** newest first */
  activity: Activity[];
  /** command id -> port learned from its output */
  ports: Record<string, number>;
}

/** library.rs */
export type Library = "all" | "favorites" | "recent" | "missing" | "new";
export type SortBy = "recent" | "name" | "kind";

export interface Query {
  search: string;
  library: Library;
  kind: Kind | null;
  sort: SortBy;
}

export interface Counts {
  all: number;
  favorites: number;
  recent: number;
  missing: number;
  new: number;
  kinds: Partial<Record<Kind, number>>;
}

/** store.rs `ScanRoot` */
export interface ScanRoot {
  path: string;
  depth: number;
}

/** state.rs `LibraryView` + `savedTo` added in commands.rs */
export interface LibraryView {
  projects: Project[];
  counts: Counts;
  scanRoots: ScanRoot[];
  scanning: boolean;
  savedTo: string;
}

/** scan_job.rs */
export interface ScanProgress {
  root: string;
  found: number;
  added: number;
}

export interface ScanSummary {
  roots: number;
  added: number;
  duplicates: number;
  ignored: number;
  unreadable: number;
  missingRoots: string[];
}

/** tools.rs `ToolScan`: resolved paths, or null when not found. */
export interface ToolScan {
  tools: { id: string; name: string; path: string | null }[];
  gitBash: string | null;
  checkedAt: number;
}

export const toolPath = (scan: ToolScan | null, id: string) =>
  scan?.tools.find((t) => t.id === id)?.path ?? null;

/** agents.rs */
export type AgentId = "claude" | "codex" | "opencode";
export type TerminalKind = "windowsTerminal" | "powerShell" | "gitBash" | "cmd";

export interface LaunchRecord {
  agent: AgentId | null;
  terminal: TerminalKind;
  subdir: string | null;
  at: number;
}

/** prefs.rs */
export interface Settings {
  editor: string;
  terminal: TerminalKind;
  agent: AgentId;
  checkPorts: boolean;
}
export interface Shortcut {
  slot: number;
  projectId: ProjectId;
  agent: AgentId | null;
  terminal: TerminalKind;
  subdir: string | null;
}

/** src-tauri agents.rs `AgentsOverview` */
export interface AgentsOverview {
  agents: { id: AgentId; name: string; command: string; path: string | null; installHint: string }[];
  terminals: { kind: TerminalKind; name: string; available: boolean }[];
  editors: { command: string; name: string; path: string | null }[];
  settings: Settings;
  shortcuts: (Shortcut & { projectName: string })[];
  tools: ToolScan;
}

/** error.rs `ErrorDto` */
export interface ErrorDto {
  kind: string;
  message: string;
  fix: string | null;
}

export type DetailTab = "overview" | "git" | "run" | "agents";

/** run/spec.rs */
export type CommandId = string;
export type CommandSource = { kind: "script"; body: string } | { kind: "detected" } | { kind: "custom" };

/** run/state.rs `RunState`, flattened into `RunInfo` */
export type RunStateTag = "idle" | "starting" | "running" | "stopping" | "stopped" | "exited" | "failed";

/** run/supervisor.rs `RunInfo` */
export interface RunInfo {
  projectId: ProjectId;
  commandId: CommandId;
  projectName: string;
  commandName: string;
  commandLine: string;
  cwd: string;
  state: RunStateTag;
  pid?: number;
  /** exited: number; failed: number or null (never started / no code) */
  code?: number | null;
  processCount: number | null;
  startedAt: number | null;
  endedAt: number | null;
  portHint: number | null;
  lastError: string | null;
}

/** runs.rs `CommandView` */
export interface RunCommand {
  id: CommandId;
  name: string;
  commandLine: string;
  cwd: string;
  source: CommandSource;
  run: RunInfo | null;
  /** Learned from an earlier run's output; checked before starting. */
  knownPort: number | null;
}

/** ports/mod.rs `ProcInfo`; null fields = Windows wouldn't tell (unknown). */
export interface ProcInfo {
  pid: number;
  parent: number | null;
  name: string;
  exe: string | null;
  cmd: string | null;
  cwd: string | null;
  /** unix seconds */
  startTime: number | null;
  session: number | null;
}

/** ports/ownership.rs */
export type Owner = "devDock" | "system" | "external";
export interface PortRow {
  port: number;
  addresses: string[];
  pid: number;
  owner: Owner;
  process: ProcInfo | null;
  children: { pid: number; name: string }[];
  project: { id: ProjectId; name: string } | null;
  run: { projectId: ProjectId; commandId: CommandId } | null;
}
export interface PortsView {
  rows: PortRow[];
  refreshedAt: number;
}

/** ports/conflict.rs */
export interface Conflict {
  port: number;
  holder: PortRow;
  suggestedPort: number | null;
}

/** runs.rs `StartOutcome` */
export type StartOutcome = { outcome: "started" } | { outcome: "conflict"; conflict: Conflict };

/** ports/confirm.rs */
export interface ConfirmRequest {
  pid: number;
  startTime: number | null;
  withChildren: boolean;
}
export interface Ended {
  pids: number[];
  name: string;
}

export interface CommandList {
  commands: RunCommand[];
  warning: string | null;
}

/** run/logs.rs */
export type Stream = "out" | "err" | "system";
export interface LogLine {
  seq: number;
  ts: number;
  stream: Stream;
  text: string;
}
export interface LogsPage {
  lines: LogLine[];
  nextSeq: number;
  truncated: boolean;
}

export interface RunChanged {
  projectId: ProjectId;
  commandId: CommandId;
  state: boolean;
  logs: boolean;
}

export const isActive = (s: RunStateTag | undefined) =>
  s === "starting" || s === "running" || s === "stopping";

export type View = "projects" | "spaces" | "running" | "ports" | "agents" | "settings";

/** workspace/model.rs */
export type WorkspaceId = string;
export type Health = { kind: "tcp"; port: number } | { kind: "http"; url: string; expect: number | null };
export type ServiceSource =
  | { kind: "command"; projectId: ProjectId; commandId: CommandId; worktree: string | null }
  | { kind: "compose"; projectId: ProjectId; file: string; service: string };
export interface Service {
  id: string;
  name: string;
  order: number;
  source: ServiceSource;
  port: number | null;
  health: Health | null;
  dependsOn: string[];
}
export interface RestartPolicy {
  enabled: boolean;
  max: number;
  windowSecs: number;
}
export interface AfterStart {
  projectId: ProjectId | null;
  openEditor: boolean;
  agent: AgentId | null;
  terminal: TerminalKind;
}

/** workspace/orchestrator.rs */
export type SvcState =
  | "pending"
  | "starting"
  | "healthy"
  | "up"
  | "unhealthy"
  | "gaveUp"
  | "failed"
  | "stopping"
  | "stopped";
export interface SvcRuntime {
  id: string;
  state: SvcState;
  startedAt: number | null;
  lastCheck: number | null;
  restarts: number[];
  message: string | null;
}
export type PhaseTag = "idle" | "starting" | "running" | "stopping" | "stopped" | "failed";
export interface LiveWorkspace {
  phase: PhaseTag;
  step?: number;
  reason?: string;
  services: SvcRuntime[];
}

/** src-tauri workspaces.rs `WorkspaceView` */
export interface WorkspaceView {
  id: WorkspaceId;
  name: string;
  services: Service[];
  restart: RestartPolicy;
  afterStart: AfterStart;
  live: LiveWorkspace | null;
  extra: Record<string, { projectName: string; detail: string; run: RunInfo | null }>;
}

/** workspace/logs.rs */
export interface MergedLine extends LogLine {
  service: string;
}
export interface MergedPage {
  lines: MergedLine[];
  cursors: Record<string, number>;
}

/** activity.rs */
export type Activity =
  | { kind: "openedInEditor"; editor: string; at: number }
  | { kind: "openedInExplorer"; at: number }
  | { kind: "started"; command: string; at: number }
  | { kind: "ended"; command: string; outcome: "stopped" | "exited" | "failed"; code: number | null; at: number }
  | { kind: "agentLaunched"; agent: AgentId | null; terminal: TerminalKind; at: number };

/** palette.rs */
export type PaletteAction =
  | { kind: "openProject"; projectId: ProjectId }
  | { kind: "runCommand"; projectId: ProjectId; commandId: CommandId }
  | { kind: "launchAgent"; projectId: ProjectId; agent: AgentId }
  | { kind: "startWorkspace"; workspaceId: WorkspaceId }
  | { kind: "stopWorkspace"; workspaceId: WorkspaceId }
  | { kind: "showPort"; port: number; pid: number }
  | { kind: "goTo"; view: View }
  | { kind: "rescan" }
  | { kind: "stopEverything" };
export interface PaletteHit {
  group: string;
  label: string;
  hint: string;
  action: PaletteAction;
  /** code-point indices in `label` that matched */
  positions: number[];
}
export interface PaletteGroup {
  group: string;
  title: string;
  hits: PaletteHit[];
}
