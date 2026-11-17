# DevDock — Product

> A native Rust desktop app that puts all your local projects, Git state, dev servers, ports and coding agents in one window.

## 1. Summary

DevDock is a local-first desktop control center for developers on Windows. It **organizes tools rather than replacing them**: it finds your projects, shows their Git state, runs and watches their dev commands, explains what is on each port, and opens your editor, terminal or AI coding agent in the right folder with one action.

DevDock is not an IDE, a Git client, a terminal emulator or an AI chat app. It sits next to those tools and gets you to the right one, in the right place, quickly.

## 2. Problem

A typical working day means switching between many local projects, and each switch costs a little:

- Finding the folder, then opening it in Explorer, VS Code and a terminal one at a time.
- Forgetting which branch a project is on, or whether it has uncommitted work.
- Starting `npm run dev` in several terminals and losing track of which is running where.
- "Port 3000 is already in use", with no quick way to see who holds it or whether it's safe to stop.
- Opening Claude Code or Codex and then having to `cd` into the correct folder first.
- Starting a frontend, API and database by hand, in the right order, every time.

## 3. Target user

- A developer working on several local projects (Next.js, Node, Rust, Docker-based services, backends such as Odoo).
- Works mainly on Windows, often with terminal-based AI coding agents.
- Wants speed and clarity, not another heavy tool. Doesn't want accounts or cloud sync.

The first user is the author, who is building DevDock to learn Rust. Product decisions should still hold up for any developer like the one described above.

## 4. Goals and non-goals

**Goals**
- One place to find and open any local project in under 3 seconds.
- Show Git, server and port state at a glance, without opening other tools.
- Run and supervise dev commands reliably, with logs, from inside DevDock.
- Make port conflicts understandable and safe to fix.
- Launch coding agents in the correct working directory with one action.
- Start and stop a set of related services as one workspace.

**Non-goals**
- Editing code, viewing diffs or writing Git history (no commits, resets or branch switching).
- Replacing the terminal or the IDE.
- An AI chat interface, or managing AI provider accounts or sessions.
- Accounts, cloud backends or syncing between machines.
- Automatically killing processes DevDock didn't start.

## 5. Principles

1. **Orchestrate, don't replace.** Every action ends in a real tool: Explorer, the editor, a terminal or an agent CLI.
2. **Local-first.** All data lives on the machine. No sign-in.
3. **Safe by default.** DevDock clearly separates processes it started from external ones. Ending an external process always needs explicit confirmation showing full details.
4. **Truthful state.** What DevDock shows (branch, file counts, process state, port owner) must match reality. When it can't know something, it says so instead of guessing.
5. **Helpful failure.** Every error says what happened and what to do next (for example: "`code` not found on PATH — reinstall VS Code with 'Add to PATH'").
6. **Small, fast, native.** Starts quickly, stays light, works with the keyboard.

## 6. Information architecture

```
DevDock window
├── Title bar ── command palette (Ctrl K) · running-count indicator
├── Rail
│   ├── Projects ── list ▸ project detail
│   │                       ├── Overview
│   │                       ├── Git
│   │                       ├── Run
│   │                       └── Agents
│   ├── Spaces (Workspaces) ── list ▸ workspace detail
│   ├── Running ── every process DevDock owns
│   ├── Ports ── listening ports, conflicts, owner
│   └── Settings
└── Status strip ── counts, detected tools
```

## 7. Features

### 7.1 Projects

| Capability | Details |
|---|---|
| Add project | Native folder picker; dropping a folder onto the window also works. |
| Scan roots | Add parent folders (e.g. `D:\dev`). DevDock scans to a set depth and skips `node_modules`, `target`, `.next`, `vendor`. |
| Detection | Project type from marker files: `package.json` (+ `next.config.*` → Next.js), `Cargo.toml`, `composer.json`, `docker-compose.yml`, `.git`. |
| Persistence | Projects, favorites and settings are saved locally (`%APPDATA%\DevDock`). JSON first, SQLite once JSON is no longer enough. |
| Organize | Search by name or path, sort (recently opened, name, type), filter by type, mark favorites. |
| Manage | Rename and remove (removing never touches the folder on disk); copy path. |
| Integrity | No duplicate entries (paths are normalized). Missing folders are flagged with **Locate…** or **Remove**. |
| Open | Explorer, editor (VS Code by default), terminal, agent. |

### 7.2 Project detail — Overview
Summary cards for Git, dev server, ports and agents. Each card leads to its tab or screen. It also shows what DevDock detected (type, repo root, remotes, scripts, workspace membership) and recent activity (opened, started, failed, agent launched).

### 7.3 Git (read-only)
- Repository root, current branch, upstream, and ahead/behind counts when an upstream exists.
- Counts of staged, modified and untracked files.
- Last commit (hash, message, author, relative time), remotes, worktrees.
- Handles projects inside a shared repository (monorepos) and Git worktrees.
- Manual refresh, with a timestamp. Reads data through the Git CLI first; `git2` is optional later.
- **Never** writes to the repository.

### 7.4 Run (dev server manager)
- Reads scripts from `package.json`; supports custom commands per project.
- Runs commands with the project folder as the working directory.
- Captures stdout and stderr together, with a filter, "follow output" and highlighted stderr lines.
- States: `running`, `stopped`, `exited (code)`, `failed (code)`.
- Stop and restart, managing the **whole process tree** on Windows, not just the shell's PID.
- Never blocks the UI. Optionally stops owned processes when DevDock closes.

### 7.5 Running
One table of every process DevDock owns, across projects and workspaces: state, project and command, PID, port, uptime, and actions (logs, stop, restart). Includes **Stop everything**.

### 7.6 Ports
- Lists listening TCP ports with PID, process name, matched project and owner (`DEVDOCK`, `EXTERNAL`, `SYSTEM`).
- **Conflict check before launch.** If the port a service needs is taken, DevDock shows who holds it and offers safe options: start on another configured port, inspect the process, or end it (with confirmation).
- **Confirmation for external processes** shows the command line, folder, start time and child processes, plus an option to also end the children.
- System processes are protected. Permission errors are reported clearly (e.g. "run as administrator").
- Ownership is never guessed from a port number or executable name alone.

### 7.7 Agents (AI workspace launcher)
- Detects the `claude`, `codex` and `opencode` executables on PATH and shows where each was found.
- Launches the chosen agent in the chosen terminal, in the project root or a chosen subfolder.
- Not installed → install guide + **Re-check**.
- Keyboard shortcuts per project (e.g. `Ctrl Alt 1` → Claude Code in the repo root).
- Optional, experimental built-in terminal (PTY) that can be popped out into its own window.

### 7.8 Workspaces
- A named group of services from one or more projects.
- Each service has a command, a port, an optional health check (TCP or HTTP) and a position in the start order.
- Start all / stop all. Each service waits for the previous one's health check before starting.
- Restart policy (e.g. restart on crash, up to 3 times in 5 minutes).
- Combined logs labelled by service, with a filter for each service.
- Optional Docker Compose services and Git worktree support.
- Presets, e.g. "open VS Code + Claude Code after start".

### 7.9 Command palette
`Ctrl K` from anywhere. Fuzzy search across projects, commands ("Run dev in DevChrono"), agent launches, workspaces and ports ("Who's using port 8069?").

### 7.10 Settings
Editor, terminal, default agent; detected tools; scan roots and depth; safety toggles (confirmation for external processes is **always on**); where data is stored, plus export/import.

## 8. Key user flows

1. **Open a project:** `Ctrl K` → type "chrono" → Enter → Overview → **VS Code**.
2. **Start working:** open the project → Run → ▶ `dev` → watch the log → the dev server card turns green.
3. **Port conflict:** start a service → the conflict banner shows `python.exe` (PID 9120) holding the port → **Start on 8070 instead**, or **End PID…** → confirm.
4. **Ask an agent:** open the project → **Claude Code** (or `Ctrl Alt 1`) → Windows Terminal opens in the right folder.
5. **Full stack:** Spaces → Wallet stack → **Start all** → postgres ▸ api ▸ web turn healthy in order → combined log.

## 9. Safety rules (must never break)

- Never end a process DevDock didn't start without explicit, detailed confirmation.
- Never touch system processes.
- Never write to a Git repository.
- Never delete project folders; "Remove" only removes the entry from DevDock.
- Always stop the whole process tree for processes DevDock owns, so no orphans are left behind.

## 10. Roadmap

This product spec describes the complete app. It is still built in increments; each version ends with a working app.

| Version | Delivers |
|---|---|
| v0.1.0 | Window, list of projects, folder picker, open in Explorer / VS Code, helpful errors |
| v0.2.0 | Persistence, scan roots, type detection, search/sort/favorites, missing-folder handling |
| v0.3.0 | Read-only Git panel |
| v0.4.0 | Run tab, process supervision, logs, Running screen |
| v0.5.0 | Ports screen, conflict detection, confirmation for external processes |
| v0.6.0 | Agents tab, terminal/editor preferences, shortcuts, experimental PTY |
| v0.7.0+ | Workspaces, health checks, restart rules, Docker Compose, worktrees |
| Later | Command palette everywhere, Git operations, diffs, terminal tabs, Docker monitoring, resource usage, Linux/macOS |

## 11. Tech direction

- **Language:** Rust. Application logic is kept separate from the UI so UI frameworks can be swapped.
- **UI:** Tauri 2 shell with a Svelte 5 (SvelteKit, SPA mode) front end. Rust owns all logic and state; Svelte only renders and sends commands. (Changed from egui on 2026-11-16, see `decisions.md`.)
- **Libraries (when needed):** `tauri-plugin-dialog`, `serde`/`serde_json`, `walkdir`, Git CLI (`git2` later), `std::process`, `sysinfo`, `portable-pty` + `xterm.js`, `tokio`, `rusqlite`.
- **Platform:** Windows first; platform-specific code (process trees, port ownership, launching) isolated behind traits.

## 12. Success criteria

- **As a product:** switching projects, checking repo state, starting servers, fixing port conflicts and opening agents each take one or two actions, and the state DevDock shows is trustworthy.
- **As a learning project:** the work covers Rust ownership, errors, modules, processes, concurrency and native UI through real features.

## 13. Open questions

- Should launch shortcuts be global (system-wide) hotkeys or only work while DevDock has focus?
- When should JSON storage move to SQLite (number of projects, log history)?
- Should logs persist between restarts, and for how long?
- Which health-check types does v0.7 need beyond TCP and HTTP GET?
