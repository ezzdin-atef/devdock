// Demo backend for README screenshots. Injected before the page loads (see
// scripts/capture-screenshots.mjs) so the real Svelte UI runs in a plain browser against
// curated data: no personal paths, no real processes. Mirrors the DTOs in src/lib/types.ts.
(() => {
  const now = Date.now();
  const min = 60_000;
  const hour = 60 * min;
  const day = 24 * hour;
  const secs = (ms) => Math.floor(ms / 1000);

  const ROOT = "D:\\dev";
  const p = (name) => `${ROOT}\\${name}`;

  const git = (branch, extra = {}) => ({
    state: "repo",
    refreshedAt: now,
    data: {
      oid: "a1b2c3d4e5f6",
      branch,
      upstream: `origin/${branch}`,
      ahead: 0,
      behind: 0,
      staged: 0,
      modified: 0,
      untracked: 0,
      conflicted: 0,
      changedFiles: 0,
      ...extra,
    },
  });

  const projects = [
    {
      id: "a000000000000001",
      name: "storefront",
      path: p("storefront"),
      kind: "nextJs",
      isGit: true,
      favorite: true,
      lastOpened: now - 12 * min,
      addedAt: now - 40 * day,
      missing: false,
      isNew: false,
      git: git("main", { ahead: 2, modified: 3, untracked: 1, changedFiles: 4 }),
      launches: [{ agent: "claude", terminal: "windowsTerminal", subdir: null, at: now - 25 * min }],
      activity: [
        { kind: "started", command: "dev", at: now - 14 * min },
        { kind: "openedInEditor", editor: "VS Code", at: now - 12 * min },
        { kind: "agentLaunched", agent: "claude", terminal: "windowsTerminal", at: now - 25 * min },
        { kind: "ended", command: "build", outcome: "exited", code: 0, at: now - 2 * hour },
      ],
      ports: { dev: 3000 },
    },
    {
      id: "a000000000000002",
      name: "api",
      path: p("api"),
      kind: "node",
      isGit: true,
      favorite: true,
      lastOpened: now - 40 * min,
      addedAt: now - 40 * day,
      missing: false,
      isNew: false,
      git: git("feat/auth-tokens", { ahead: 1, staged: 2, changedFiles: 2 }),
      launches: [],
      activity: [{ kind: "started", command: "dev", at: now - 14 * min }],
      ports: { dev: 4000 },
    },
    {
      id: "a000000000000003",
      name: "billing-service",
      path: p("billing-service"),
      kind: "rust",
      isGit: true,
      favorite: false,
      lastOpened: now - 3 * hour,
      addedAt: now - 30 * day,
      missing: false,
      isNew: false,
      git: git("main"),
      launches: [],
      activity: [],
      ports: {},
    },
    {
      id: "a000000000000004",
      name: "docs",
      path: p("docs"),
      kind: "nextJs",
      isGit: true,
      favorite: false,
      lastOpened: now - 1 * day,
      addedAt: now - 30 * day,
      missing: false,
      isNew: false,
      git: git("main", { behind: 3 }),
      launches: [],
      activity: [],
      ports: {},
    },
    {
      id: "a000000000000005",
      name: "infra",
      path: p("infra"),
      kind: "compose",
      isGit: true,
      favorite: false,
      lastOpened: now - 2 * day,
      addedAt: now - 30 * day,
      missing: false,
      isNew: false,
      git: git("main", { modified: 1, changedFiles: 1 }),
      launches: [],
      activity: [],
      ports: {},
    },
    {
      id: "a000000000000006",
      name: "legacy-portal",
      path: p("legacy-portal"),
      kind: "php",
      isGit: true,
      favorite: false,
      lastOpened: now - 9 * day,
      addedAt: now - 30 * day,
      missing: false,
      isNew: false,
      git: git("release/2.4"),
      launches: [],
      activity: [],
      ports: {},
    },
    {
      id: "a000000000000007",
      name: "design-notes",
      path: p("design-notes"),
      kind: "unknown",
      isGit: false,
      favorite: false,
      lastOpened: null,
      addedAt: now - 2 * day,
      missing: false,
      isNew: false,
      git: { state: "noRepo", refreshedAt: now },
      launches: [],
      activity: [],
      ports: {},
    },
  ];
  const byId = Object.fromEntries(projects.map((x) => [x.id, x]));
  const [storefront, api] = projects;

  const run = (proj, commandId, commandLine, port, startedAgo, pid) => ({
    projectId: proj.id,
    commandId,
    projectName: proj.name,
    commandName: commandId,
    commandLine,
    cwd: proj.path,
    state: "running",
    pid,
    processCount: 4,
    startedAt: now - startedAgo,
    endedAt: null,
    portHint: port,
    lastError: null,
  });
  const runs = [
    run(storefront, "dev", "pnpm run dev", 3000, 14 * min, 18244),
    run(api, "dev", "pnpm run dev", 4000, 14 * min, 18310),
  ];

  const cmd = (proj, id, body, extra = {}) => ({
    id,
    name: id,
    commandLine: `pnpm run ${id}`,
    cwd: proj.path,
    source: { kind: "script", body },
    run: runs.find((r) => r.projectId === proj.id && r.commandId === id) ?? null,
    knownPort: null,
    ...extra,
  });
  const commands = {
    [storefront.id]: [
      cmd(storefront, "dev", "next dev --turbopack", { knownPort: 3000 }),
      cmd(storefront, "build", "next build", {
        run: {
          ...run(storefront, "build", "pnpm run build", null, 2 * hour, 0),
          state: "exited",
          code: 0,
          endedAt: now - 2 * hour + 48_000,
          processCount: null,
        },
      }),
      cmd(storefront, "start", "next start"),
      cmd(storefront, "lint", "next lint"),
      cmd(storefront, "test", "vitest run"),
      {
        id: "storybook",
        name: "storybook",
        commandLine: "pnpm dlx storybook dev -p 6006",
        cwd: storefront.path,
        source: { kind: "custom" },
        run: null,
        knownPort: 6006,
      },
    ],
    [api.id]: [
      cmd(api, "dev", "tsx watch src/server.ts", { knownPort: 4000 }),
      cmd(api, "build", "tsc -p ."),
      cmd(api, "test", "vitest run"),
    ],
  };

  const t0 = now - 14 * min;
  const logText = [
    ["system", "> pnpm run dev  (D:\\dev\\storefront)"],
    ["out", ""],
    ["out", "> storefront@0.9.0 dev D:\\dev\\storefront"],
    ["out", "> next dev --turbopack"],
    ["out", ""],
    ["out", "   \u25b2 Next.js 15.4.2 (Turbopack)"],
    ["out", "   - Local:        http://localhost:3000"],
    ["out", "   - Network:      http://192.168.1.24:3000"],
    ["out", "   - Environments: .env.local"],
    ["out", ""],
    ["out", " \u2713 Starting..."],
    ["out", " \u2713 Ready in 1184ms"],
    ["out", " \u25cb Compiling / ..."],
    ["out", " \u2713 Compiled / in 2.3s"],
    ["out", " GET / 200 in 2514ms"],
    ["out", " \u25cb Compiling /products/[slug] ..."],
    ["out", " \u2713 Compiled /products/[slug] in 612ms"],
    ["out", " GET /products/linen-shirt 200 in 702ms"],
    ["err", " \u26a0 Image with src \"/hero.jpg\" has \"fill\" but is missing \"sizes\" prop."],
    ["out", " GET /cart 200 in 88ms"],
    ["out", " GET /api/cart 200 in 41ms"],
    ["out", " \u2713 Compiled /checkout in 431ms"],
    ["out", " GET /checkout 200 in 486ms"],
  ];
  const logs = logText.map(([stream, text], i) => ({
    seq: i + 1,
    ts: t0 + i * 1700 + (i > 12 ? 60_000 * (i - 12) : 0),
    stream,
    text,
  }));

  const proc = (pid, name, exe, cmdLine, cwd, startAgo, parent = 4) => ({
    pid,
    parent,
    name,
    exe,
    cmd: cmdLine,
    cwd,
    startTime: secs(now - startAgo),
    session: 1,
  });
  const sys = (port, pid, name, addresses = ["0.0.0.0", "::"]) => ({
    port,
    addresses,
    pid,
    owner: "system",
    process: proc(pid, name, `C:\\Windows\\System32\\${name}`, null, null, 3 * day),
    children: [],
    project: null,
    run: null,
  });
  const ext = (port, pid, name, exe, cmdLine, project = null) => ({
    port,
    addresses: ["127.0.0.1"],
    pid,
    owner: "external",
    process: proc(pid, name, exe, cmdLine, null, 5 * hour, 2200),
    children: [],
    project,
    run: null,
  });
  const own = (port, r) => ({
    port,
    addresses: ["0.0.0.0", "::"],
    pid: r.pid + 12,
    owner: "devDock",
    process: proc(r.pid + 12, "node.exe", "C:\\Program Files\\nodejs\\node.exe", r.commandLine, r.cwd, 14 * min, r.pid),
    children: [],
    project: { id: r.projectId, name: r.projectName },
    run: { projectId: r.projectId, commandId: r.commandId },
  });
  const portRows = [
    sys(135, 1180, "svchost.exe"),
    sys(445, 4, "System"),
    own(3000, runs[0]),
    own(4000, runs[1]),
    ext(5432, 6120, "com.docker.backend.exe", "C:\\Program Files\\Docker\\Docker\\resources\\com.docker.backend.exe", "com.docker.backend.exe run", { id: "a000000000000005", name: "infra" }),
    sys(5040, 7316, "svchost.exe"),
    ext(5173, 22408, "node.exe", "C:\\Program Files\\nodejs\\node.exe", "node vite --port 5173"),
    ext(6379, 6120, "com.docker.backend.exe", "C:\\Program Files\\Docker\\Docker\\resources\\com.docker.backend.exe", "com.docker.backend.exe run", { id: "a000000000000005", name: "infra" }),
    sys(7680, 6344, "svchost.exe"),
    ext(8080, 14872, "php.exe", "C:\\tools\\php\\php.exe", "php -S localhost:8080 -t public", { id: "a000000000000006", name: "legacy-portal" }),
    ext(9229, 22408, "node.exe", "C:\\Program Files\\nodejs\\node.exe", "node --inspect vite --port 5173"),
    sys(49664, 968, "lsass.exe"),
  ];

  const tools = {
    tools: [
      ["explorer", "Explorer", "C:\\Windows\\explorer.exe"],
      ["git", "Git", "C:\\Program Files\\Git\\cmd\\git.exe"],
      ["code", "VS Code", "C:\\Program Files\\Microsoft VS Code\\bin\\code.cmd"],
      ["cursor", "Cursor", "C:\\Program Files\\Cursor\\resources\\app\\bin\\cursor.cmd"],
      ["code-insiders", "VS Code Insiders", null],
      ["windsurf", "Windsurf", null],
      ["zed", "Zed", "C:\\Program Files\\Zed\\bin\\zed.exe"],
      ["wt", "Windows Terminal", "C:\\Program Files\\WindowsApps\\Microsoft.WindowsTerminal\\wt.exe"],
      ["pwsh", "PowerShell 7", "C:\\Program Files\\PowerShell\\7\\pwsh.exe"],
      ["powershell", "Windows PowerShell", "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"],
      ["cmd", "Command Prompt", "C:\\Windows\\System32\\cmd.exe"],
      ["docker", "Docker", "C:\\Program Files\\Docker\\Docker\\resources\\bin\\docker.exe"],
      ["claude", "Claude Code", "C:\\Program Files\\nodejs\\claude.cmd"],
      ["codex", "Codex", "C:\\Program Files\\nodejs\\codex.cmd"],
      ["opencode", "OpenCode", "C:\\Program Files\\nodejs\\opencode.cmd"],
    ].map(([id, name, path]) => ({ id, name, path })),
    gitBash: "C:\\Program Files\\Git\\bin\\bash.exe",
    checkedAt: now - 3 * min,
  };
  const toolPath = (id) => tools.tools.find((t) => t.id === id)?.path ?? null;
  const settings = { editor: "code", terminal: "windowsTerminal", agent: "claude", checkPorts: true };
  const agentsOverview = {
    agents: [
      { id: "claude", name: "Claude Code", command: "claude", path: toolPath("claude"), installHint: "npm install -g @anthropic-ai/claude-code" },
      { id: "codex", name: "Codex", command: "codex", path: toolPath("codex"), installHint: "npm install -g @openai/codex" },
      { id: "opencode", name: "OpenCode", command: "opencode", path: toolPath("opencode"), installHint: "npm install -g opencode-ai" },
    ],
    terminals: [
      { kind: "windowsTerminal", name: "Windows Terminal", available: true },
      { kind: "powerShell", name: "PowerShell", available: true },
      { kind: "gitBash", name: "Git Bash", available: true },
      { kind: "cmd", name: "Command Prompt", available: true },
    ],
    editors: [
      ["code", "VS Code"],
      ["cursor", "Cursor"],
      ["code-insiders", "VS Code Insiders"],
      ["windsurf", "Windsurf"],
      ["zed", "Zed"],
    ].map(([command, name]) => ({ command, name, path: toolPath(command) })),
    settings,
    shortcuts: [
      { slot: 1, projectId: storefront.id, agent: "claude", terminal: "windowsTerminal", subdir: null, projectName: "storefront" },
      { slot: 2, projectId: api.id, agent: "codex", terminal: "windowsTerminal", subdir: null, projectName: "api" },
    ],
    tools,
  };

  const workspaces = [
    {
      id: "w1",
      name: "Storefront stack",
      services: [],
      restart: { enabled: true, max: 3, windowSecs: 300 },
      afterStart: { projectId: storefront.id, openEditor: true, agent: null, terminal: "windowsTerminal" },
      live: null,
      extra: {},
    },
  ];

  function library(query) {
    const sorted = [...projects].sort((a, b) => (b.lastOpened ?? 0) - (a.lastOpened ?? 0));
    const kinds = {};
    for (const x of projects) kinds[x.kind] = (kinds[x.kind] ?? 0) + 1;
    let list = sorted;
    if (query?.library === "favorites") list = list.filter((x) => x.favorite);
    if (query?.kind) list = list.filter((x) => x.kind === query.kind);
    if (query?.search) list = list.filter((x) => x.name.includes(query.search.toLowerCase()));
    return {
      projects: list,
      counts: { all: projects.length, favorites: 2, recent: 6, missing: 0, new: 0, kinds },
      scanRoots: [{ path: ROOT, depth: 2 }],
      scanning: false,
      savedTo: "%APPDATA%\\DevDock\\devdock.json",
    };
  }

  // Greedy subsequence match, like the real palette.
  function match(label, q) {
    const positions = [];
    const l = label.toLowerCase();
    const at = l.indexOf(q.toLowerCase());
    if (q && at >= 0) return [...q].map((_, i) => at + i);
    let from = 0;
    for (const ch of q.toLowerCase()) {
      if (ch === " ") continue;
      const i = l.indexOf(ch, from);
      if (i < 0) return null;
      positions.push(i);
      from = i + 1;
    }
    return positions;
  }
  function palette(query) {
    const groups = [
      ["projects", "Projects", projects.map((x) => ({ label: x.name, hint: x.kind === "unknown" ? "FOLDER" : x.kind === "nextJs" ? "NEXT.JS" : x.kind.toUpperCase(), action: { kind: "openProject", projectId: x.id } }))],
      ["commands", "Commands", Object.entries(commands).flatMap(([pid, cs]) => cs.map((c) => ({ label: `Run ${c.name} in ${byId[pid].name}`, hint: c.commandLine, action: { kind: "runCommand", projectId: pid, commandId: c.id } })))],
      ["agents", "Agents", projects.slice(0, 3).flatMap((x) => agentsOverview.agents.map((a) => ({ label: `${a.name} in ${x.name}`, hint: a.command, action: { kind: "launchAgent", projectId: x.id, agent: a.id } })))],
      ["workspaces", "Workspaces", workspaces.map((w) => ({ label: `Start ${w.name}`, hint: "workspace", action: { kind: "startWorkspace", workspaceId: w.id } }))],
      ["ports", "Ports", portRows.filter((r) => r.owner !== "system").map((r) => ({ label: `Port ${r.port} \u00b7 ${r.process.name}`, hint: `PID ${r.pid}`, action: { kind: "showPort", port: r.port, pid: r.pid } }))],
    ];
    return groups
      .map(([group, title, hits]) => ({
        group,
        title,
        hits: hits
          .map((h) => {
            const positions = query ? match(h.label, query) : [];
            return positions ? { ...h, group, positions } : null;
          })
          .filter(Boolean)
          .slice(0, 3),
      }))
      .filter((g) => g.hits.length);
  }

  const repoInfo = (proj) => ({
    root: proj.path,
    isWorktree: false,
    inSubfolder: false,
    status: proj.git.data,
    lastCommit: { hash: "4f9c2e1", subject: "feat(cart): persist cart across sessions", author: "Sam Rivera", relative: "2 hours ago" },
    remotes: [{ name: "origin", url: `https://github.com/acme/${proj.name}.git` }],
    worktrees: [{ path: proj.path, branch: proj.git.data.branch, detached: false, bare: false, isCurrent: true }],
  });

  const handlers = {
    startup_notice: () => null,
    library: ({ query }) => library(query),
    refresh_projects: () => null,
    tools_scan: () => tools,
    tools_recheck: () => tools,
    agents_overview: () => agentsOverview,
    project_folders: () => ["src", "src\\app", "public", "tests"],
    git_refresh: () => null,
    git_details: ({ id }) => (byId[id]?.git?.state === "repo" ? { state: "repo", data: repoInfo(byId[id]), refreshedAt: now } : { state: "noRepo", refreshedAt: now }),
    mark_seen: () => false,
    run_commands: ({ projectId }) => ({ commands: commands[projectId] ?? [], warning: null }),
    runs_list: () => runs,
    run_logs: ({ since }) => ({ lines: since ? logs.filter((l) => l.seq >= since) : logs, nextSeq: logs.length + 1, truncated: false }),
    ports_snapshot: () => ({ rows: portRows, refreshedAt: now }),
    workspaces_list: () => workspaces,
    workspace_logs: () => ({ lines: [], cursors: {} }),
    palette_open: () => palette(""),
    palette_search: ({ query }) => palette(query),
    project_worktrees: () => [],
    "plugin:app|version": () => "0.3.0",
    "plugin:app|name": () => "DevDock",
    "plugin:updater|check": () => null,
  };

  // --- IPC plumbing (same shape as @tauri-apps/api/mocks) ---
  const callbacks = new Map();
  const listeners = new Map();
  let nextId = 1;
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
    transformCallback(cb, once = false) {
      const id = nextId++;
      callbacks.set(id, (data) => {
        if (once) callbacks.delete(id);
        return cb && cb(data);
      });
      return id;
    },
    unregisterCallback: (id) => callbacks.delete(id),
    runCallback: (id, data) => callbacks.get(id)?.(data),
    callbacks,
    convertFileSrc: (path) => path,
    async invoke(cmd, args = {}) {
      if (cmd === "plugin:event|listen") {
        if (!listeners.has(args.event)) listeners.set(args.event, []);
        listeners.get(args.event).push(args.handler);
        return args.handler;
      }
      if (cmd.startsWith("plugin:event|")) return null;
      const h = handlers[cmd];
      if (h) return structuredClone(h(args));
      if (cmd.startsWith("plugin:")) return null;
      console.warn("[demo] unhandled command", cmd, args);
      return null;
    },
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
  try {
    localStorage.setItem("devdock.theme", "dark");
  } catch {}
})();
