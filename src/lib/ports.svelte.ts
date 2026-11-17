// UI state for the Ports screen and the "end external process" dialog (M5).

import * as api from "./api";
import { plural } from "./format";
import type { ErrorDto, Owner, PortRow } from "./types";

export type PortFilter = "all" | Owner;

const POLL_MS = 3000;

class PortsUi {
  rows = $state<PortRow[]>([]);
  refreshedAt = $state<number | null>(null);
  filter = $state<PortFilter>("all");
  /** Port to scroll to and mark (from the command palette). */
  highlight = $state<number | null>(null);
  loading = $state(false);
  error = $state<ErrorDto | null>(null);
  /** Plain-language result of the last end/stop, shown under the header. */
  notice = $state<string | null>(null);

  /** The process shown in the inspect/end dialog. */
  inspecting = $state<PortRow | null>(null);
  withChildren = $state(true);
  ending = $state(false);
  dialogError = $state<ErrorDto | null>(null);
  /** Called after a successful end (the conflict banner retries its start). */
  private afterEnd: (() => void) | null = null;

  shown = $derived(this.filter === "all" ? this.rows : this.rows.filter((r) => r.owner === this.filter));
  counts = $derived({
    all: this.rows.length,
    devDock: this.rows.filter((r) => r.owner === "devDock").length,
    external: this.rows.filter((r) => r.owner === "external").length,
    system: this.rows.filter((r) => r.owner === "system").length,
  });

  private timer: ReturnType<typeof setInterval> | null = null;
  private inFlight = false;

  async refresh() {
    if (this.inFlight) return;
    this.inFlight = true;
    this.loading = true;
    try {
      const view = await api.portsSnapshot();
      this.rows = view.rows;
      this.refreshedAt = view.refreshedAt;
      this.error = null;
    } catch (err) {
      this.error = api.toErrorDto(err);
    } finally {
      this.loading = false;
      this.inFlight = false;
    }
  }

  /** Poll every 3 s while the Ports screen is visible (T5.4). */
  startPolling() {
    this.refresh();
    this.timer ??= setInterval(() => this.refresh(), POLL_MS);
  }

  stopPolling() {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
  }

  inspect(row: PortRow, afterEnd?: () => void) {
    this.inspecting = row;
    this.withChildren = row.children.length > 0;
    this.dialogError = null;
    this.afterEnd = afterEnd ?? null;
  }

  closeDialog() {
    this.inspecting = null;
    this.dialogError = null;
    this.afterEnd = null;
  }

  /** The user pressed "End process" in the confirmation dialog. */
  async endConfirmed() {
    const row = this.inspecting;
    if (!row || this.ending) return;
    this.ending = true;
    try {
      const ended = await api.endProcess({
        pid: row.pid,
        startTime: row.process?.startTime ?? null,
        withChildren: this.withChildren,
      });
      const extra = ended.pids.length - 1;
      this.notice = `Ended ${ended.name} (PID ${row.pid})${extra > 0 ? ` and ${plural(extra, "child process", "child processes")}` : ""}. Port ${row.port} is free unless something else takes it.`;
      const next = this.afterEnd;
      this.closeDialog();
      await this.refresh();
      next?.();
    } catch (err) {
      this.dialogError = api.toErrorDto(err);
    } finally {
      this.ending = false;
    }
  }
}

export const ports = new PortsUi();
