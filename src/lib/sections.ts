// Splits the project list into sections: by the user's groups, by the folder each
// project sits in on disk, or not at all. Shared by the list and the home view.

import type { Group, GroupColor, ListGrouping, Project } from "./types";
import { KIND_NAME, UNGROUPED } from "./types";

export interface Section {
  /** Stable key (group id, folder path, or "all"); also remembers collapse. */
  key: string;
  label: string;
  /** Longer text for a tooltip (the full folder path). */
  title?: string;
  color?: GroupColor;
  /** Set for group sections (UNGROUPED for "Ungrouped"): a drop target. */
  groupId?: string;
  projects: Project[];
}

/** `D:\dev\work\shop` -> `D:\dev\work` (the folder it sits in). */
export function parentOf(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, "");
  const i = Math.max(trimmed.lastIndexOf("\\"), trimmed.lastIndexOf("/"));
  return i > 0 ? trimmed.slice(0, i) : trimmed;
}

/** The last two parts of a folder: `C:\Users\me\projects\work` -> `projects\work`. */
function folderLabel(path: string): string {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  return parts.length <= 2 ? path : parts.slice(-2).join("\\");
}

/**
 * `showEmpty`: list groups with no projects too (so they can be dropped on).
 * Off while searching or filtering, when empty sections would only be noise.
 */
export function buildSections(
  projects: Project[],
  groups: Group[],
  grouping: ListGrouping,
  showEmpty: boolean,
): Section[] {
  if (grouping === "none") return [{ key: "all", label: "All projects", projects }];

  if (grouping === "groups") {
    const known = new Set(groups.map((g) => g.id));
    const out: Section[] = groups.map((g) => ({
      key: `group:${g.id}`,
      label: g.name,
      color: g.color,
      groupId: g.id,
      projects: projects.filter((p) => p.group === g.id),
    }));
    out.push({
      key: "group:-",
      label: groups.length ? "Ungrouped" : "All projects",
      groupId: UNGROUPED,
      projects: projects.filter((p) => !p.group || !known.has(p.group)),
    });
    return out.filter((s) => s.projects.length > 0 || (showEmpty && s.groupId !== UNGROUPED));
  }

  if (grouping === "kind") {
    const byKind = new Map<string, Project[]>();
    for (const p of projects) {
      const list = byKind.get(p.kind);
      if (list) list.push(p);
      else byKind.set(p.kind, [p]);
    }
    return [...byKind.entries()]
      .map(([kind, list]) => ({
        key: `kind:${kind}`,
        label: KIND_NAME[kind as Project["kind"]] ?? kind,
        projects: list,
      }))
      .sort((a, b) => a.label.localeCompare(b.label));
  }

  // Folders on disk, in the order their first project appears (keeps the sort).
  const byFolder = new Map<string, Project[]>();
  for (const p of projects) {
    const f = parentOf(p.path);
    const list = byFolder.get(f.toLowerCase());
    if (list) list.push(p);
    else byFolder.set(f.toLowerCase(), [p]);
  }
  return [...byFolder.values()].map((list) => {
    const folder = parentOf(list[0].path);
    return { key: `folder:${folder.toLowerCase()}`, label: folderLabel(folder), title: folder, projects: list };
  });
}

const STORE = "devdock.collapsed";

/** Which sections are collapsed: kept in the webview's storage, per section key. */
export function loadCollapsed(): Record<string, true> {
  try {
    return JSON.parse(localStorage.getItem(STORE) ?? "{}");
  } catch {
    return {};
  }
}

export function saveCollapsed(c: Record<string, true>) {
  try {
    localStorage.setItem(STORE, JSON.stringify(c));
  } catch {
    // Storage full or blocked: collapse still works for this session.
  }
}

/** Colour values for the named group colours (readable on light and dark). */
export const GROUP_HEX: Record<string, string> = {
  red: "#e5484d",
  orange: "#ea7a2b",
  amber: "#d4a017",
  yellow: "#e2c400",
  lime: "#84cc16",
  green: "#22a35a",
  emerald: "#10b981",
  teal: "#14a3a3",
  cyan: "#06b6d4",
  sky: "#38bdf8",
  blue: "#3b82f6",
  indigo: "#6366f1",
  violet: "#8b5cf6",
  purple: "#a855f7",
  pink: "#ec4899",
  rose: "#e5487a",
  slate: "#7c8796",
  stone: "#a8a29e",
};

/** The CSS colour for a group colour: a named one, or a custom `#rrggbb` as is. */
export function colorOf(c: string | undefined): string {
  if (!c) return "var(--line-strong)";
  return GROUP_HEX[c] ?? (/^#[0-9a-f]{6}$/i.test(c) ? c : "var(--line-strong)");
}
