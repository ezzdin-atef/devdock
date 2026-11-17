# DevDock

A local-first Windows desktop control panel for the projects on your machine. Find them, see
their Git state, run their dev servers, see who holds which port, start coding agents in them,
and bring up a whole workspace of services in order - all from one window.

DevDock **organizes your tools instead of replacing them**. It is not an IDE, a Git client, a
terminal or an AI chat app; every action ends in a real tool (Explorer, your editor, a terminal,
an agent CLI) opened in the right folder.

![Overview](docs/screenshots/overview.png)

- **Website:** [ezzdin-atef.github.io/devdock](https://ezzdin-atef.github.io/devdock/)
- **Version:** 0.3.0 ([changelog](CHANGELOG.md))
- **Platform:** Windows 10/11 (x64)
- **Stack:** Rust + Tauri 2, Svelte 5 UI
- **License:** [MIT](LICENSE)

## Contents

- [Why](#why)
- [Features](#features)
- [Install](#install)
- [Quick start](#quick-start)
- [Keyboard](#keyboard)
- [Safety rules](#safety-rules)
- [Data and configuration](#data-and-configuration)
- [Build from source](#build-from-source)
- [Checks and tests](#checks-and-tests)
- [Architecture](#architecture)
- [Releasing](#releasing)
- [Troubleshooting](#troubleshooting)
- [Documentation](#documentation)

## Why

A normal working day across several local projects costs small amounts of time, over and over:

- Finding the folder, then opening Explorer, the editor and a terminal one by one.
- Forgetting which branch a project is on, or whether it has uncommitted work.
- Running `npm run dev` in several terminals and losing track of what runs where.
- "Port 3000 is already in use", with no quick way to see who holds it or whether it's safe to stop.
- Opening Claude Code or Codex, then `cd`-ing into the right folder first.
- Starting a database, API and frontend by hand, in the right order, every time.

DevDock turns each of these into one or two actions. No accounts, no cloud, no sync.

## Features

### Projects

- Add a folder with the picker (or drop it onto the window), or add **scan roots** such as
  `D:\dev` and let DevDock find projects below them up to a chosen depth. Scans skip
  `node_modules`, `target`, `.next`, `vendor` and `.git`.
- **Type detection** from marker files:

  | Type | Detected by |
  |---|---|
  | Next.js | `package.json` + `next.config.*` |
  | Node | `package.json` |
  | Rust | `Cargo.toml` |
  | PHP | `composer.json` |
  | Docker Compose | `docker-compose.yml` / `compose.yml` (and `.yaml`) |

- Search by name or path, sort (recent, name, type), filter by type, favourites.
- Rename, remove (never touches the folder on disk), copy path. Paths are normalized, so the
  same folder can't be added twice.
- Missing folders are flagged with **Locate...** or **Remove**.
- Open in Explorer, your editor (VS Code, Cursor, VS Code Insiders, Windsurf or Zed), a
  terminal, or an agent.

### Git (read-only)

- Repository root, branch, upstream, ahead/behind.
- Staged, modified and untracked counts; last commit (hash, message, author, time); remotes;
  worktrees.
- Works for projects inside a shared repository (monorepos) and for Git worktrees.
- Reads through the `git` CLI and **never writes** to the repository.

### Run

- Lists `package.json` scripts (run with the project's package manager - npm, pnpm, yarn or
  bun, picked from the lockfile) and lets you add custom commands per project.
- Each run lives in a **Windows Job Object**, so stopping it - or quitting DevDock - ends the
  whole process tree, not just the shell. No orphaned `node.exe` left behind.
- Live console with stdout and stderr together, filter, follow output, highlighted stderr.
- States: running, stopped, exited (code), failed (code). Exit codes and detected ports are shown.
- The **Running** screen lists every process DevDock owns across projects and workspaces, with
  logs, stop, restart and **Stop everything**.

![Run](docs/screenshots/run.png)

### Ports

- Every listening TCP port with its PID, process, matched project and owner:
  `DEVDOCK`, `EXTERNAL` or `SYSTEM`.
- **Conflict check before launch**: if a port a service needs is taken, DevDock shows who holds
  it and offers to start elsewhere, inspect the process, or end it.
- Ending an external process always asks first and shows its command line, folder, start time
  and child processes. System processes are never offered.

![Ports](docs/screenshots/ports.png)

### Agents

- Detects **Claude Code** (`claude`), **Codex** (`codex`) and **OpenCode** (`opencode`) on
  `PATH` and shows where each was found. Missing ones show an install hint and **Re-check**.
- Launches the agent in **Windows Terminal**, **PowerShell**, **Git Bash** or **Command Prompt**,
  in the project root or a chosen subfolder.
- Bind up to nine launches to `Ctrl Alt 1`-`9` (works while DevDock is focused).

### Workspaces

A named group of services from one or more projects, started as one unit:

- Each service is either a project command (optionally in one of its Git worktrees) or a
  **Docker Compose** service.
- Start order, optional explicit dependencies, and a port for conflict checks.
- **Health checks**: TCP (something accepts a connection on the port) or HTTP `GET` (expects a
  given status, or any 2xx/3xx). Each service waits for the previous one to be healthy.
- **Restart policy**: restart on crash, by default up to 3 times in 5 minutes.
- Combined logs labelled by service, with per-service filters.
- **After start** preset: open the editor and/or an agent in a project once everything is up.

### Command palette

`Ctrl K` from anywhere: fuzzy search across projects, commands ("run dev in api"), agent
launches, workspaces and ports ("who's on 8069?").

![Palette](docs/screenshots/palette.png)

### Settings and app

- Editor, terminal and default agent; detected tools; scan roots and depth.
- Dark and light themes, following Windows or chosen manually (toggle in the title bar).
- Data location, export and import.
- In-app updates from GitHub Releases (installed builds only).

![Settings](docs/screenshots/settings.png)

## Install

Download `DevDock_x.y.z_x64-setup.exe` from
[Releases](https://github.com/ezzdin-atef/devdock/releases/latest) and run it.

- Installs per-machine into `C:\Program Files\DevDock` (asks for admin once).
- Adds a Start menu entry and registers `devdock` so it starts from `Win+R`.
- Installs the WebView2 runtime if it's missing (it's built into Windows 11).
- Updates itself from then on: an **Update x.y.z** pill appears in the title bar when a new
  version is out.

The installer isn't Authenticode-signed yet, so SmartScreen may show "Windows protected your
PC" on first run: **More info -> Run anyway**. In-app updates are signed and verified.

To uninstall, use **Settings -> Apps**. The uninstaller offers to delete DevDock's data too.

### Optional tools

DevDock uses what's already on your machine:

| Tool | Used for |
|---|---|
| `git` | Git panel |
| An editor CLI (`code`, `cursor`, `code-insiders`, `windsurf`, `zed`) | Open in editor |
| Windows Terminal (`wt`), PowerShell, Git Bash | Terminal and agent launches (`cmd` always works) |
| `claude`, `codex`, `opencode` | Agents |
| Docker (with Compose) | Compose services in workspaces |
| Node package managers (npm, pnpm, yarn, bun) | Running package scripts |

Settings shows which of these were detected and where.

## Quick start

1. **Add projects** - Projects -> add a folder, or Settings -> add a scan root like `D:\dev`.
2. **Open one** - `Ctrl K`, type part of its name, `Enter`. Click **VS Code** (or your editor).
3. **Run it** - Run tab -> start `dev`. Watch the console; the port appears once it's listening.
4. **Fix a port clash** - if the port is taken, the banner shows who holds it; pick another port
   or end the process after reviewing its details.
5. **Ask an agent** - Agents tab -> **Claude Code**. A terminal opens in the project folder.
6. **Bring up a stack** - Spaces -> create a workspace, add services (e.g. db, api, web) with
   health checks -> **Start all**.

## Keyboard

| Key | Does |
|---|---|
| `Ctrl K` | Open / close the command palette |
| `Up` / `Down` | Move through the project or workspace list |
| `Enter` | Open the selected project's Overview, or run the palette hit |
| `Esc` | Close the palette or a dialog |
| `F5` | Refresh the current screen |
| `Ctrl Alt 1`-`9` | Run a bound agent / terminal launch |

## Safety rules

These are product invariants, not settings:

- Never end a process DevDock didn't start without explicit confirmation showing full details.
- Never touch system processes.
- Never write to a Git repository.
- Never delete project folders - **Remove** only removes the entry from DevDock.
- Always stop the whole process tree of processes DevDock owns. Quitting while runs are active
  asks first and then stops them all.
- Never guess port ownership from a port number or executable name alone.

## Data and configuration

All state lives in one JSON file, `devdock.json`, in `%APPDATA%\DevDock`:

- Projects, favourites, custom commands, scan roots, workspaces, agent shortcuts and settings.
- Saved atomically (write then rename), debounced, with a `.bak` copy of the previous version.
- If the file is damaged, DevDock renames it to `devdock.json.corrupt-<time>` (never
  overwrites it), falls back to the `.bak`, and warns you.
- **Settings -> Data** shows the location and exports / imports the file.

| Environment variable | Effect |
|---|---|
| `DEVDOCK_DATA_DIR` | Use this folder for data instead of `%APPDATA%\DevDock` |

To try DevDock without touching your real project list:

```powershell
$env:DEVDOCK_DATA_DIR = "C:\temp\devdock-demo"; pnpm tauri dev
```

## Build from source

### Prerequisites

- Windows 10/11
- [Rust](https://rustup.rs) stable with the MSVC toolchain (edition 2024)
- Node 20+ and [pnpm](https://pnpm.io) (the version is pinned in `package.json`)
- WebView2 runtime (built into Windows 11)
- Visual Studio Build Tools with "Desktop development with C++" (needed by the MSVC toolchain)

### Commands

Install JS dependencies:

```bash
pnpm install
```

Run in development (hot reload for the UI, rebuilds Rust on change):

```bash
pnpm tauri dev
```

Build a release binary and the NSIS installer:

```bash
pnpm tauri build
```

The binary lands in `target/release/devdock.exe`, the installer in
`target/release/bundle/nsis/`.

## Checks and tests

The same checks CI runs on `windows-latest` for every push to `main` and every PR:

```bash
cargo fmt --all --check
```

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

```bash
cargo test --workspace
```

```bash
pnpm check
```

```bash
pnpm contrast
```

- `cargo test` includes integration tests that create real Git repositories
  (`crates/devdock-core/tests/git_repos.rs`) and check terminal launch plans
  (`tests/terminals.rs`), so `git` must be on `PATH`.
- `pnpm check` runs `svelte-check` (TypeScript + Svelte).
- `pnpm contrast` verifies theme colours meet WCAG AA in both themes.

Lint policy: `unsafe_code` is forbidden workspace-wide (except the platform crate),
`clippy::unwrap_used` is denied and `expect_used` warns.

## Architecture

```
+-------------------------------+
|  Svelte 5 UI (src/)           |  renders DTOs, sends commands
|  src/lib/api.ts (typed calls) |
+---------------+---------------+
                | Tauri commands + events
+---------------v---------------+
|  src-tauri/                   |  thin command/event layer
+---------------+---------------+
                |
+---------------v---------------+     +------------------------------+
|  crates/devdock-core          |---->|  crates/devdock-platform     |
|  all domain logic and state   |     |  Windows FFI: Job Objects,   |
|  (no Tauri dependency)        |     |  TCP table, process details  |
+-------------------------------+     +------------------------------+
```

**Rust owns all state and all user-facing error text**; the UI only renders and sends commands.
Platform-specific code is isolated behind traits so the core stays testable and portable.

| Path | What it is |
|---|---|
| `crates/devdock-core` | Domain logic: projects, detection, scanning, git, runs, ports, agents, workspaces, palette, persistence. |
| `crates/devdock-platform` | Windows FFI (Job Objects, TCP table, process details). The only crate allowed `unsafe`. |
| `src-tauri` | Tauri app: commands, events, window, updater, installer config. |
| `src` | Svelte 5 (SvelteKit, SPA mode) UI. Screens in `src/lib/screens`, shared components in `src/lib/components`. |
| `scripts` | Release, changelog and contrast-check scripts. |
| `docs` | Product spec, design system, plan, tasks, decisions, states audit, release guide. |
| `.github/workflows` | `ci.yml` (checks + unsigned build), `release.yml` (signed release on `v*` tags). |

Key modules in `devdock-core`:

| Module | Responsibility |
|---|---|
| `detect`, `scan`, `library`, `project` | Project types, scan roots, the project list |
| `git/` | Read-only Git facts via the CLI, with caching |
| `run/` | Command specs, process supervision, logs, port hints |
| `ports/` | Port ownership, conflicts, end-process confirmation |
| `agents`, `launcher`, `tools` | Tool detection on `PATH`, terminal and agent launch plans |
| `workspace/` | Service model, start order, health checks, restart policy, combined logs |
| `palette` | Command palette search |
| `store`, `saver`, `state` | In-memory state and atomic JSON persistence |

## Releasing

```bash
pnpm release patch
```

Bumps the version (`Cargo.toml` is the single source of truth), writes `CHANGELOG.md` from
conventional commit subjects, commits, tags and pushes. GitHub Actions then builds, signs and
publishes the installer and the in-app update.

| Command | Result |
|---|---|
| `pnpm release patch` / `minor` / `major` | Semver bump |
| `pnpm release beta` | Pre-release (skipped by the updater) |
| `pnpm release 0.5.0` | Exact version |
| `pnpm release:dry` | Preview the next patch release, change nothing |
| `pnpm build:installer` | Build an installer locally without publishing |

One-time setup (signing key, secrets) and details: [docs/RELEASING.md](docs/RELEASING.md).

## Troubleshooting

| Problem | Fix |
|---|---|
| "`code` not found on PATH" | Reinstall the editor with "Add to PATH", or pick another editor in Settings. |
| An agent shows "not installed" | Install it (e.g. `npm install -g @anthropic-ai/claude-code`), then **Re-check**. |
| Can't end a process on the Ports screen | It may need elevation; run DevDock as administrator. System processes are never offered. |
| Git panel is empty | Make sure `git` is on `PATH` and the folder is inside a repository. |
| SmartScreen warning on install | The installer isn't code-signed yet: **More info -> Run anyway**. |
| Startup warning about recovered data | The data file was damaged; the original is kept as `devdock.json.corrupt-<time>` in `%APPDATA%\DevDock`. |

## Documentation

- [docs/PRODUCT.md](docs/PRODUCT.md) - product spec, goals, flows, roadmap
- [docs/DESIGN.md](docs/DESIGN.md) - design system and UI rules
- [docs/PLAN.md](docs/PLAN.md) - architecture and implementation plan
- [docs/TASKS.md](docs/TASKS.md) - task breakdown
- [docs/decisions.md](docs/decisions.md) - why things are the way they are
- [docs/states-audit.md](docs/states-audit.md) - empty, loading and error states
- [docs/RELEASING.md](docs/RELEASING.md) - release process and updater
- [CHANGELOG.md](CHANGELOG.md) - release history

## License

[MIT](LICENSE)
