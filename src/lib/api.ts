// The only place the UI talks to Rust. Components import from here, never `invoke` directly.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  PaletteGroup,
  AfterStart,
  MergedPage,
  RestartPolicy,
  Service,
  WorkspaceId,
  WorkspaceView,
  CommandId,
  ConfirmRequest,
  Ended,
  PortsView,
  StartOutcome,
  CommandList,
  ErrorDto,
  LogsPage,
  RunChanged,
  RunInfo,
  LibraryView,
  RepoInfo,
  Snapshot,
  Project,
  ProjectId,
  Query,
  ScanProgress,
  ScanSummary,
  ToolScan,
  AgentId,
  AgentsOverview,
  Settings,
  Shortcut,
  TerminalKind,
} from "./types";

export const startupNotice = () => invoke<ErrorDto | null>("startup_notice");
export const library = (query: Query) => invoke<LibraryView>("library", { query });
export const addProject = (path: string) => invoke<Project>("add_project", { path });
export const removeProject = (id: ProjectId) => invoke<void>("remove_project", { id });
export const renameProject = (id: ProjectId, name: string) =>
  invoke<void>("rename_project", { id, name });
export const setFavorite = (id: ProjectId, favorite: boolean) =>
  invoke<void>("set_favorite", { id, favorite });
export const relocateProject = (id: ProjectId, path: string) =>
  invoke<ProjectId>("relocate_project", { id, path });
export const refreshProjects = () => invoke<void>("refresh_projects");
export const toolsScan = () => invoke<ToolScan>("tools_scan");
export const toolsRecheck = () => invoke<ToolScan>("tools_recheck");
export const agentsOverview = () => invoke<AgentsOverview>("agents_overview");
export const projectFolders = (projectId: ProjectId) =>
  invoke<string[]>("project_folders", { projectId });
export const launchAgent = (
  projectId: ProjectId,
  agent: AgentId | null,
  terminal: TerminalKind,
  subdir: string | null,
) => invoke<void>("launch_agent", { projectId, agent, terminal, subdir });
export const setSettings = (settings: Settings) => invoke<void>("set_settings", { settings });
export const setShortcut = (shortcut: Shortcut) => invoke<void>("set_shortcut", { shortcut });
export const removeShortcut = (slot: number) => invoke<void>("remove_shortcut", { slot });
export const launchShortcut = (slot: number) => invoke<string>("launch_shortcut", { slot });
export const openInExplorer = (id: ProjectId) => invoke<void>("open_in_explorer", { id });
export const openInEditor = (id: ProjectId) => invoke<void>("open_in_editor", { id });
export const addScanRoot = (path: string) => invoke<void>("add_scan_root", { path });
export const removeScanRoot = (path: string) => invoke<void>("remove_scan_root", { path });
export const rescan = () => invoke<void>("rescan");
/** Background refresh; results arrive as `git:updated` events. Empty ids = all projects. */
export const gitRefresh = (ids: ProjectId[], details: boolean) =>
  invoke<void>("git_refresh", { ids, details });
export const gitDetails = (id: ProjectId) =>
  invoke<Snapshot<RepoInfo> | null>("git_details", { id });

export const runCommands = (projectId: ProjectId) =>
  invoke<CommandList>("run_commands", { projectId });
/** Checks the command's known port first; returns a conflict instead of starting. */
export const runStart = (
  projectId: ProjectId,
  commandId: CommandId,
  opts: { force?: boolean; port?: number } = {},
) =>
  invoke<StartOutcome>("run_start", {
    projectId,
    commandId,
    force: opts.force ?? null,
    port: opts.port ?? null,
  });
export const portsSnapshot = () => invoke<PortsView>("ports_snapshot");
/** Only after the user confirmed in the dialog; Rust re-checks the PID. */
export const endProcess = (request: ConfirmRequest) => invoke<Ended>("end_process", { request });
export const runStop = (projectId: ProjectId, commandId: CommandId) =>
  invoke<void>("run_stop", { projectId, commandId });
export const runRestart = (projectId: ProjectId, commandId: CommandId) =>
  invoke<void>("run_restart", { projectId, commandId });
export const runLogs = (projectId: ProjectId, commandId: CommandId, since: number) =>
  invoke<LogsPage | null>("run_logs", { projectId, commandId, since });
export const runsList = () => invoke<RunInfo[]>("runs_list");
export const runStopAll = () => invoke<void>("run_stop_all");
export const addCommand = (
  projectId: ProjectId,
  name: string,
  commandLine: string,
  subdir: string | null,
) => invoke<CommandId>("add_command", { projectId, name, commandLine, subdir });
export const removeCommand = (projectId: ProjectId, commandId: CommandId) =>
  invoke<void>("remove_command", { projectId, commandId });
export const quitApp = () => invoke<void>("quit_app");

export const onRunChanged = (cb: (c: RunChanged) => void): Promise<UnlistenFn> =>
  listen<RunChanged>("run:changed", (e) => cb(e.payload));
export const onCloseRequested = (cb: (active: number) => void): Promise<UnlistenFn> =>
  listen<number>("app:close-requested", (e) => cb(e.payload));

export const workspacesList = () => invoke<WorkspaceView[]>("workspaces_list");
export const workspaceCreate = (name: string) => invoke<WorkspaceId>("workspace_create", { name });
export const workspaceRename = (id: WorkspaceId, name: string) =>
  invoke<void>("workspace_rename", { id, name });
export const workspaceDelete = (id: WorkspaceId) => invoke<void>("workspace_delete", { id });
export const projectWorktrees = (projectId: ProjectId) =>
  invoke<string[]>("project_worktrees", { projectId });
export const workspaceUpsertService = (id: WorkspaceId, service: Service) =>
  invoke<void>("workspace_upsert_service", { id, service });
export const workspaceRemoveService = (id: WorkspaceId, serviceId: string) =>
  invoke<void>("workspace_remove_service", { id, serviceId });
export const workspaceSetRules = (id: WorkspaceId, restart: RestartPolicy, afterStart: AfterStart) =>
  invoke<void>("workspace_set_rules", { id, restart, afterStart });
export const workspaceStart = (id: WorkspaceId) => invoke<void>("workspace_start", { id });
export const workspaceStop = (id: WorkspaceId) => invoke<void>("workspace_stop", { id });
export const workspaceLogs = (id: WorkspaceId, cursors: Record<string, number>) =>
  invoke<MergedPage>("workspace_logs", { id, cursors });
export const onWorkspaceChanged = (cb: (id: WorkspaceId) => void): Promise<UnlistenFn> =>
  listen<WorkspaceId>("workspace:changed", (e) => cb(e.payload));
export const onPresetError = (cb: (e: ErrorDto) => void): Promise<UnlistenFn> =>
  listen<ErrorDto>("workspace:preset-error", (e) => cb(e.payload));

export const paletteOpen = () => invoke<PaletteGroup[]>("palette_open");
export const paletteSearch = (query: string) => invoke<PaletteGroup[]>("palette_search", { query });

export const setScanRootDepth = (path: string, depth: number) =>
  invoke<void>("set_scan_root_depth", { path, depth });
export const exportData = (path: string) => invoke<void>("export_data", { path });
export const importPreview = (path: string) =>
  invoke<{ projects: number; scanRoots: number; workspaces: number; shortcuts: number; currentProjects: number }>(
    "import_preview",
    { path },
  );
export const importData = (path: string) => invoke<void>("import_data", { path });

/** Native "save as" for exports. Resolves to null when cancelled. */
export async function pickSaveFile(defaultName: string): Promise<string | null> {
  const picked = await save({ title: "Export DevDock data", defaultPath: defaultName, filters: [{ name: "JSON", extensions: ["json"] }] });
  return picked ?? null;
}

/** Native "open" for imports. */
export async function pickJsonFile(): Promise<string | null> {
  const picked = await open({ multiple: false, directory: false, title: "Import DevDock data", filters: [{ name: "JSON", extensions: ["json"] }] });
  return typeof picked === "string" ? picked : null;
}

export const onScanProgress = (cb: (p: ScanProgress) => void): Promise<UnlistenFn> =>
  listen<ScanProgress>("scan:progress", (e) => cb(e.payload));
export const onScanDone = (cb: (s: ScanSummary) => void): Promise<UnlistenFn> =>
  listen<ScanSummary>("scan:done", (e) => cb(e.payload));
export const onGitUpdated = (
  cb: (u: { id: ProjectId; details: boolean }) => void,
): Promise<UnlistenFn> => listen<{ id: ProjectId; details: boolean }>("git:updated", (e) => cb(e.payload));
export const onStoreError = (cb: (e: ErrorDto) => void): Promise<UnlistenFn> =>
  listen<ErrorDto>("store:error", (e) => cb(e.payload));

/** Native folder picker. Resolves to null when the user cancels. */
export async function pickFolder(title: string): Promise<string | null> {
  const picked = await open({ directory: true, multiple: false, title });
  return typeof picked === "string" ? picked : null;
}

/** Commands reject with an ErrorDto; anything else (IPC failure) is wrapped so the banner can show it. */
export function toErrorDto(err: unknown): ErrorDto {
  if (err && typeof err === "object" && "message" in err && "kind" in err) {
    return err as ErrorDto;
  }
  return { kind: "unexpected", message: String(err), fix: null };
}
