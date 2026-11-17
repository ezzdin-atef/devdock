// Display helpers only. No domain decisions here.

const UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
  ["year", 365 * 24 * 3600e3],
  ["month", 30 * 24 * 3600e3],
  ["week", 7 * 24 * 3600e3],
  ["day", 24 * 3600e3],
  ["hour", 3600e3],
  ["minute", 60e3],
];

const rtf = new Intl.RelativeTimeFormat("en", { numeric: "auto" });

/** "3 hours ago", "yesterday", "just now". */
export function relativeTime(ms: number, now = Date.now()): string {
  const diff = ms - now;
  for (const [unit, size] of UNITS) {
    if (Math.abs(diff) >= size) return rtf.format(Math.round(diff / size), unit);
  }
  return "just now";
}

export function shortDate(ms: number): string {
  return new Date(ms).toLocaleDateString("en-GB", { day: "numeric", month: "short", year: "numeric" });
}

/**
 * Keeps the start (drive) and the end (the folders that identify the project) of a
 * long path: `C:\Users\me\AppData\...\dev\wallet\api` -> `C:\…\dev\wallet\api`.
 */
export function shortPath(path: string, tail = 3): string {
  const sep = path.includes("\\") ? "\\" : "/";
  const parts = path.split(sep).filter((p, i) => p !== "" || i === 0);
  if (parts.length <= tail + 2) return path;
  return [parts[0], "…", ...parts.slice(-tail)].join(sep);
}

export function plural(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** Uptime: "42s", "4m 12s", "2h 03m". */
export function duration(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${String(s % 60).padStart(2, "0")}s`;
  return `${Math.floor(m / 60)}h ${String(m % 60).padStart(2, "0")}m`;
}

/** "14:32:05" for log timestamps. */
export function logTime(ms: number): string {
  return new Date(ms).toLocaleTimeString("en-GB", { hour12: false });
}

/** "14:32", for "refreshed hh:mm" stamps. */
export function clockTime(ms: number): string {
  return new Date(ms).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" });
}
