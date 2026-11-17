# Empty and error states audit (T8.5)

Each row of DESIGN.md section 8, where it lives, and how it was checked. "Live" means seen in
the running app during the milestone that built it; "unit" means covered by a Rust test.

| Situation | Implementation | Checked |
|---|---|---|
| Empty project list | Dashed "Choose a folder to dock…" and "Add scan root" targets (`ProjectList.svelte`); detail says what to do | Live (M1, M2) |
| Missing folder | Muted row, dashed red dot, red mono "Folder not found"; detail panel with **Locate…** / **Remove** and the "nothing on disk is touched" note; Missing count in the sidebar | Live (M2), unit (`refresh_flags_missing_folders`, relocate tests) |
| Tool missing | Status strip shows `<editor> missing` / `git missing`; launching shows the error banner with the fix ("Install … with Add to PATH, or pick another editor in Settings") | Unit (`missing_editor_is_reported…`), live (M1) |
| Not a Git repo | List subline "no git"; Git tab explains and hides the counters; Overview tile "no git" | Live (M3) |
| No upstream | Git tab "no upstream" instead of ↑/↓; Overview tile "· no upstream" | Live (M3), unit (`no_upstream_means_unknown_ahead_behind`) |
| Loading / scanning | Inline muted text: "Scanning <root>… N found", "Reading the repository…", "Gathering projects, commands and ports…"; never a blocking spinner | Live (M2, M3, M8) |
| Process failed | `FAILED <code>` pill, last error line panel, **Run again** as the primary button; card "failed 1 · just now"; Running table row tinted; activity "failed (exit 1)" | Live (M4, M8) |
| Permission denied | End-process dialog shows "Windows denied access to <name> (PID n). It belongs to another user or runs elevated." + "Run DevDock as administrator, or end it from Task Manager." Processes Windows won't describe are shown as SYSTEM / Protected instead of offering an End that would fail | Unit (`access_denied_is_reported`), live (M5 refusal paths) |

Additional states found while auditing, all implemented:

| Situation | Implementation |
|---|---|
| Port already in use before start | Conflict banner: port tile, who holds it, safe option first, inspect, destructive last (M5) |
| Damaged data file | Restored from `.bak`, damaged file kept as `.corrupt-<time>`, one-time notice (M2) |
| Agent not installed | Dashed card, install command, Re-check (M6) |
| Workspace failed to start | Phase FAILED with the reason ("api: not ready after 60 s (…)"), later services not started (M7) |
| Import while processes run | Refused with "Stop everything, then import again" (M8) |
| Nothing matches in the palette | "Nothing matches “…”." (M8) |

Copy follows DESIGN.md's voice: it names the real command, path, port or PID, says what DevDock
did or didn't do, and has no exclamation marks.
