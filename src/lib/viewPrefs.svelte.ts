// How the Projects page looks: layout, card size, grouping, sort and which details
// each project shows. Saved in the app's own storage, so it's there next time.

import type { ListGrouping, SortBy } from "./types";

/** `folders`: one card per group/folder; `cards`: one card per project; `rows`: compact rows. */
export type Layout = "folders" | "cards" | "rows";
export type Size = "s" | "m" | "l";

export interface Details {
  /** Branch and uncommitted changes. */
  git: boolean;
  /** Running commands. */
  status: boolean;
  /** "not installed" / "update packages". */
  packages: boolean;
  /** Critical/high findings or secrets from the last security scan. */
  security: boolean;
  /** Next.js, Vite, Rust... */
  kind: boolean;
  /** Folder on disk. */
  path: boolean;
  /** When it was last opened from DevDock. */
  opened: boolean;
}

export const DETAIL_LABELS: { key: keyof Details; label: string }[] = [
  { key: "status", label: "Running state" },
  { key: "git", label: "Git branch and changes" },
  { key: "packages", label: "Packages status" },
  { key: "security", label: "Security findings" },
  { key: "kind", label: "Project type" },
  { key: "opened", label: "Last opened" },
  { key: "path", label: "Folder path" },
];

interface Saved {
  layout: Layout;
  size: Size;
  grouping: ListGrouping;
  sort: SortBy;
  details: Details;
  favoritesRow: boolean;
}

const KEY = "devdock.view";
// Every project in one grid by default; grouping is something the user turns on.
const DEFAULTS: Saved = {
  layout: "cards",
  size: "m",
  grouping: "none",
  sort: "recent",
  details: { git: true, status: true, packages: true, security: true, kind: true, path: false, opened: false },
  favoritesRow: true,
};

function load(): Saved {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    return { ...DEFAULTS, ...raw, details: { ...DEFAULTS.details, ...(raw.details ?? {}) } };
  } catch {
    return structuredClone(DEFAULTS);
  }
}

class ViewPrefs {
  private saved = load();
  layout = $state<Layout>(this.saved.layout);
  size = $state<Size>(this.saved.size);
  grouping = $state<ListGrouping>(this.saved.grouping);
  sort = $state<SortBy>(this.saved.sort);
  details = $state<Details>(this.saved.details);
  favoritesRow = $state(this.saved.favoritesRow);

  set(patch: Partial<Saved>) {
    Object.assign(this, patch);
    this.save();
  }

  toggleDetail(key: keyof Details) {
    this.details[key] = !this.details[key];
    this.save();
  }

  reset() {
    this.set(structuredClone(DEFAULTS));
  }

  private save() {
    try {
      const data: Saved = {
        layout: this.layout,
        size: this.size,
        grouping: this.grouping,
        sort: this.sort,
        details: $state.snapshot(this.details),
        favoritesRow: this.favoritesRow,
      };
      localStorage.setItem(KEY, JSON.stringify(data));
    } catch {
      // Storage blocked: the choice still holds for this session.
    }
  }
}

export const view = new ViewPrefs();
