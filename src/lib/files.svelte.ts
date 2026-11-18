// Files tab state, kept per project for the whole session: the tree's open folders,
// the search, and the open file with its editor state. Leaving the tab (or the
// project) keeps unsaved edits; closing DevDock with unsaved edits asks first.

import type { EditorState } from "@codemirror/state";
import * as api from "./api";
import type { ErrorDto, FileFormat, Listing, ProjectId, SearchQuery, SearchResult } from "./types";

export interface OpenFile {
  path: string;
  format: FileFormat;
  /** Last-modified time of the version on disk we last read or wrote. */
  mtime: number;
  dirty: boolean;
  /** The file changed or went away on disk while open. */
  conflict: "changed" | "deleted" | null;
  saving: boolean;
  /** Where to put the cursor once the editor shows (from a search hit). */
  reveal: { line: number; from: number; to: number } | null;
  /** Bumped when the text is replaced from disk, so the editor reloads it. */
  version: number;
}

/** A file couldn't open (too large, binary…): shown in place of the editor. */
export interface Unopenable {
  path: string;
  error: ErrorDto;
}

export interface ProjectFiles {
  panel: "tree" | "search";
  expanded: Record<string, true>;
  listings: Record<string, Listing>;
  loading: Record<string, true>;
  open: OpenFile | null;
  failed: Unopenable | null;
  /** Switching away from a dirty file waits for Save / Discard / Cancel. */
  pending: { path: string; reveal: OpenFile["reveal"] } | null;
  query: SearchQuery;
  result: SearchResult | null;
  searching: boolean;
  error: ErrorDto | null;
}

/** Editor state lives outside `$state`: it changes on every keystroke. */
interface Buffer {
  /** Text as last read from or written to disk. */
  saved: string;
  state: EditorState | null;
  /** The text to load when no state exists yet. */
  text: string;
}

function fresh(): ProjectFiles {
  return {
    panel: "tree",
    expanded: {},
    listings: {},
    loading: {},
    open: null,
    failed: null,
    pending: null,
    query: { text: "", regex: false, matchCase: false, wholeWord: false, include: "", exclude: "", hidden: false },
    result: null,
    searching: false,
    error: null,
  };
}

const HIDDEN_KEY = "devdock.files.showHidden";

function loadShowHidden(): boolean {
  try {
    // On unless turned off: .env, .gitignore and .github are files you work on.
    return localStorage.getItem(HIDDEN_KEY) !== "0";
  } catch {
    return true;
  }
}

class FilesUi {
  byProject = $state<Record<ProjectId, ProjectFiles>>({});
  /** Show dotfiles, `.git` and Windows-hidden items in the tree and search. Remembered. */
  showHidden = $state(loadShowHidden());
  private buffers = new Map<ProjectId, Buffer>();
  private searchSeq = 0;

  /** Projects with unsaved edits, for the tab dot and the quit dialog. */
  unsaved = $derived(
    Object.entries(this.byProject)
      .filter(([, f]) => f.open?.dirty)
      .map(([id, f]) => ({ id, path: f.open!.path })),
  );

  constructor() {
    $effect.root(() => {
      $effect(() => {
        api.filesSetUnsaved(this.unsaved.length > 0).catch(() => {});
      });
    });
  }

  /** Creates the project's state if needed. Not from `$derived` or markup: it writes state. */
  ensure(id: ProjectId): ProjectFiles {
    if (!this.byProject[id]) this.byProject[id] = fresh();
    return this.byProject[id];
  }

  /** Read-only view for markup; FilesTab calls `ensure` when it mounts. */
  of(id: ProjectId): ProjectFiles {
    return this.byProject[id] ?? fresh();
  }

  buffer(id: ProjectId): Buffer | undefined {
    return this.buffers.get(id);
  }

  // ---- tree ----

  async loadDir(id: ProjectId, dir: string) {
    const f = this.ensure(id);
    f.loading[dir] = true;
    try {
      f.listings[dir] = await api.filesList(id, dir, this.showHidden);
    } catch (err) {
      f.error = api.toErrorDto(err);
    } finally {
      delete f.loading[dir];
    }
  }

  async toggleDir(id: ProjectId, dir: string) {
    const f = this.ensure(id);
    if (f.expanded[dir]) {
      delete f.expanded[dir];
      return;
    }
    f.expanded[dir] = true;
    if (!f.listings[dir]) await this.loadDir(id, dir);
  }

  /** Flips "show hidden", re-reads the project's tree and re-runs its search. */
  async setShowHidden(id: ProjectId, show: boolean) {
    this.showHidden = show;
    try {
      localStorage.setItem(HIDDEN_KEY, show ? "1" : "0");
    } catch {
      // Not saved; it still applies until DevDock closes.
    }
    // Other projects' trees were read with the old setting: drop them.
    for (const [pid, f] of Object.entries(this.byProject)) {
      if (pid !== id) f.listings = {};
    }
    await this.refreshTree(id);
    if (this.ensure(id).query.text) await this.search(id);
  }

  /** Re-reads every folder that's showing. */
  async refreshTree(id: ProjectId) {
    const f = this.ensure(id);
    const dirs = ["", ...Object.keys(f.expanded)];
    f.listings = {};
    await Promise.all(dirs.map((d) => this.loadDir(id, d)));
  }

  /** Opens the folders leading to `path` so it shows in the tree. */
  private async expandTo(id: ProjectId, path: string) {
    const f = this.ensure(id);
    const parts = path.split("/").slice(0, -1);
    for (let i = 1; i <= parts.length; i++) {
      const dir = parts.slice(0, i).join("/");
      f.expanded[dir] = true;
      if (!f.listings[dir]) await this.loadDir(id, dir);
    }
  }

  // ---- open / save ----

  /** Opens `path`; with unsaved edits in another file, asks first. */
  async open(id: ProjectId, path: string, reveal: OpenFile["reveal"] = null) {
    const f = this.ensure(id);
    if (f.open && f.open.path === path) {
      if (reveal) f.open.reveal = reveal;
      return;
    }
    if (f.open?.dirty) {
      f.pending = { path, reveal };
      return;
    }
    await this.load(id, path, reveal);
  }

  private async load(id: ProjectId, path: string, reveal: OpenFile["reveal"]) {
    const f = this.ensure(id);
    f.pending = null;
    try {
      const file = await api.fileRead(id, path);
      this.buffers.set(id, { saved: file.text, text: file.text, state: null });
      f.failed = null;
      f.open = {
        path,
        format: file.format,
        mtime: file.mtime,
        dirty: false,
        conflict: null,
        saving: false,
        reveal,
        version: (f.open?.version ?? 0) + 1,
      };
      this.expandTo(id, path);
    } catch (err) {
      this.buffers.delete(id);
      f.open = null;
      f.failed = { path, error: api.toErrorDto(err) };
    }
  }

  /** The editor reports every change; dirty = differs from what's on disk. */
  changed(id: ProjectId, state: EditorState) {
    const b = this.buffers.get(id);
    const f = this.byProject[id];
    if (!b || !f?.open) return;
    b.state = state;
    const dirty = state.doc.length !== b.saved.length || state.doc.toString() !== b.saved;
    if (dirty !== f.open.dirty) f.open.dirty = dirty;
  }

  /** Keeps the editor state when the editor unmounts (tab or project switch). */
  keep(id: ProjectId, state: EditorState) {
    const b = this.buffers.get(id);
    if (b) b.state = state;
  }

  async save(id: ProjectId, force = false): Promise<boolean> {
    const f = this.ensure(id);
    const b = this.buffers.get(id);
    const open = f.open;
    if (!open || !b || open.saving) return false;
    const text = b.state ? b.state.doc.toString() : b.text;
    open.saving = true;
    try {
      open.mtime = await api.fileWrite(id, open.path, text, open.format, open.mtime, force);
      b.saved = text;
      open.dirty = b.state ? b.state.doc.toString() !== text : false;
      open.conflict = null;
      return true;
    } catch (err) {
      const e = api.toErrorDto(err);
      if (e.kind === "fileChanged") open.conflict = "changed";
      else if (e.kind === "fileNotFound") open.conflict = "deleted";
      else f.error = e;
      return false;
    } finally {
      open.saving = false;
    }
  }

  /** Throws away edits and reads the file again from disk. */
  async reload(id: ProjectId) {
    const f = this.ensure(id);
    if (!f.open) return;
    const path = f.open.path;
    f.open.dirty = false;
    await this.load(id, path, null);
  }

  /** Answer to "unsaved changes": save or discard, then open what was asked for. */
  async resolvePending(id: ProjectId, choice: "save" | "discard" | "cancel") {
    const f = this.ensure(id);
    const next = f.pending;
    if (!next || choice === "cancel") {
      f.pending = null;
      return;
    }
    if (choice === "save" && !(await this.save(id))) return;
    if (f.open) f.open.dirty = false;
    await this.load(id, next.path, next.reveal);
  }

  close(id: ProjectId) {
    const f = this.ensure(id);
    if (f.open?.dirty) return;
    f.open = null;
    f.failed = null;
    this.buffers.delete(id);
  }

  /**
   * On window focus: did the open file change on disk? A clean file reloads
   * quietly; one with edits shows the conflict bar instead.
   */
  async checkDisk(id: ProjectId) {
    const f = this.byProject[id];
    const open = f?.open;
    if (!open || open.saving) return;
    try {
      const mtime = await api.fileStat(id, open.path);
      if (f.open !== open) return;
      if (mtime === null) open.conflict = "deleted";
      else if (mtime !== open.mtime) {
        if (open.dirty) open.conflict = "changed";
        else await this.load(id, open.path, null);
      }
    } catch {
      // The project itself went away; the detail view shows that.
    }
  }

  // ---- search ----

  async search(id: ProjectId) {
    const f = this.ensure(id);
    const seq = ++this.searchSeq;
    if (!f.query.text) {
      f.result = null;
      f.searching = false;
      return;
    }
    f.searching = true;
    f.error = null;
    try {
      const result = await api.filesSearch(id, { ...$state.snapshot(f.query), hidden: this.showHidden });
      if (seq !== this.searchSeq || result.cancelled) return;
      f.result = result;
    } catch (err) {
      if (seq === this.searchSeq) f.error = api.toErrorDto(err);
    } finally {
      if (seq === this.searchSeq) f.searching = false;
    }
  }
}

export const files = new FilesUi();
