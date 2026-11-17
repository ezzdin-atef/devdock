// In-app updates from GitHub Releases (tauri-plugin-updater). DevDock never installs
// on its own: it checks, tells you a version is ready, and installs when you say so.
//
// Flow: check() -> "available" -> install() downloads with progress -> the NSIS
// installer runs in passive mode (Windows asks for admin once, because DevDock is
// installed per-machine in Program Files) -> DevDock restarts on the new version.

import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

export type UpdateStatus =
  | "idle"
  | "checking"
  | "current"
  | "available"
  | "downloading"
  | "installing"
  | "error"
  | "unsupported";

const FIRST_CHECK_MS = 8_000;
const RECHECK_MS = 6 * 3600_000;
const SKIP_KEY = "devdock.update.skipped";

class UpdaterUi {
  status = $state<UpdateStatus>("idle");
  current = $state<string | null>(null);
  version = $state<string | null>(null);
  notes = $state<string>("");
  date = $state<string | null>(null);
  error = $state<string | null>(null);
  checkedAt = $state<number | null>(null);
  received = $state(0);
  total = $state<number | null>(null);
  /** The dialog is open (from the title-bar pill, Settings, or a manual check). */
  open = $state(false);

  /** A version you chose to skip: no title-bar pill for it (Settings still shows it). */
  skipped = $state<string | null>(readSkipped());

  private update: Update | null = null;

  showPill = $derived(
    (this.status === "available" && this.skipped !== this.version) ||
      this.status === "downloading" ||
      this.status === "installing",
  );
  progress = $derived(this.total ? Math.min(1, this.received / this.total) : null);

  init() {
    getVersion()
      .then((v) => (this.current = v))
      .catch(() => {});
    // Dev builds are never the installed app; checking would only report errors.
    if (import.meta.env.DEV) {
      this.status = "unsupported";
      return;
    }
    setTimeout(() => this.check(true), FIRST_CHECK_MS);
    setInterval(() => this.check(true), RECHECK_MS);
  }

  /** `quiet`: background check; a version you skipped stays hidden and errors stay silent. */
  async check(quiet = false) {
    if (this.status === "checking" || this.status === "downloading" || this.status === "installing") return;
    if (this.status === "unsupported") {
      if (!quiet) this.open = true;
      return;
    }
    this.status = "checking";
    this.error = null;
    try {
      const found = await check();
      this.checkedAt = Date.now();
      if (!found) {
        this.update = null;
        this.status = "current";
        return;
      }
      this.update = found;
      this.version = found.version;
      this.notes = (found.body ?? "").trim();
      this.date = found.date ?? null;
      this.status = "available";
      if (!quiet) this.open = true;
    } catch (err) {
      this.checkedAt = Date.now();
      this.status = quiet ? "idle" : "error";
      this.error = describe(err);
    }
  }

  skip() {
    this.skipped = this.version;
    try {
      if (this.version) localStorage.setItem(SKIP_KEY, this.version);
    } catch {
      /* storage unavailable: skipped for this session only */
    }
    this.open = false;
  }

  async install() {
    if (!this.update) return;
    this.status = "downloading";
    this.received = 0;
    this.total = null;
    try {
      await this.update.downloadAndInstall((e) => {
        if (e.event === "Started") this.total = e.data.contentLength ?? null;
        else if (e.event === "Progress") this.received += e.data.chunkLength;
        else if (e.event === "Finished") this.status = "installing";
      });
      // On Windows the installer usually exits DevDock itself; relaunch covers the rest.
      await relaunch();
    } catch (err) {
      this.status = "error";
      this.error = describe(err);
    }
  }
}

function readSkipped(): string | null {
  try {
    return localStorage.getItem(SKIP_KEY);
  } catch {
    return null;
  }
}

function describe(err: unknown): string {
  const text = typeof err === "string" ? err : err instanceof Error ? err.message : JSON.stringify(err);
  if (/signature|pubkey|public key/i.test(text))
    return "The update's signature didn't match this build, so DevDock didn't install it.";
  if (/404|not found/i.test(text)) return "No release information was found on GitHub yet.";
  if (/network|dns|connect|timed out|offline/i.test(text))
    return "Couldn't reach GitHub. Check your connection and try again.";
  return text;
}

export const updater = new UpdaterUi();
