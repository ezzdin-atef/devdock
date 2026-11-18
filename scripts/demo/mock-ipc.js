// Demo backend for README and website screenshots. Injected before the page loads (see
// scripts/capture-screenshots.mjs) so the real Svelte UI runs in a plain browser against
// curated data: no personal paths, no real processes. Every name, path and number is made
// up. Mirrors the DTOs in src/lib/types.ts.
(() => {
  const now = Date.now();
  const min = 60e3;
  const hr = 3600e3;
  const day = 24 * hr;
  const root = "D:\\dev";

  const repo = (branch, changed = 0, ahead = 0, behind = 0) => ({
    state: "repo",
    refreshedAt: now,
    data: { oid: "a41c9e2", branch, upstream: `origin/${branch}`, ahead, behind, staged: 0, modified: changed, untracked: 0, conflicted: 0, changedFiles: changed },
  });
  const deps = (manager, state, lockfile) => ({ manager, state, command: `${manager} install`, lockfile });

  const groups = [
    { id: "g-client", name: "Client work", color: "blue" },
    { id: "g-platform", name: "Platform", color: "violet" },
    { id: "g-side", name: "Side projects", color: "green" },
  ];

  const P = (id, name, kind, group, extra = {}) => ({
    id, name, path: `${root}\\${name}`, kind, isGit: true, favorite: false,
    lastOpened: now - Number(id.slice(1)) * 5 * hr, addedAt: now - 40 * day, missing: false, isNew: false,
    git: repo("main"), launches: [], activity: [], ports: {}, commands: [], runPrefs: {},
    domain: null, hidePort: null, host: `${name}.localhost`, deps: deps("pnpm", "installed", "pnpm-lock.yaml"), group,
    ...extra,
  });

  const projects = [
    P("p1", "storefront", "nextJs", "g-client", {
      favorite: true, lastOpened: now - 12 * min, git: repo("main", 3, 1),
      activity: [
        { kind: "started", command: "pnpm run dev", at: now - 9 * min },
        { kind: "agentLaunched", agent: "claude", terminal: "windowsTerminal", at: now - 25 * min },
        { kind: "openedInEditor", editor: "code", at: now - 31 * min },
        { kind: "ended", command: "pnpm run build", outcome: "exited", code: 0, at: now - 2 * hr },
      ],
      launches: [{ agent: "claude", terminal: "windowsTerminal", subdir: null, at: now - 25 * min }],
    }),
    P("p2", "billing-api", "node", "g-platform", { git: repo("feat/invoices", 4, 1, 3), lastOpened: now - 40 * min }),
    P("p3", "admin-dashboard", "nextJs", "g-client", { deps: deps("pnpm", "outdated", "pnpm-lock.yaml"), git: repo("develop", 0, 0, 2) }),
    P("p4", "marketing-site", "node", "g-client", { git: repo("main", 1) }),
    P("p5", "auth-service", "rust", "g-platform", { deps: null, git: repo("main") }),
    P("p6", "search-indexer", "rust", "g-platform", { deps: null, git: repo("perf/batching", 2, 3) }),
    P("p7", "infra", "compose", "g-platform", { deps: null, git: repo("main") }),
    P("p8", "recipe-app", "node", "g-side", { deps: deps("npm", "missing", "package-lock.json"), git: repo("main") }),
    P("p9", "portfolio", "nextJs", "g-side", { favorite: true, git: repo("main") }),
    P("p10", "cli-tools", "rust", "g-side", { deps: null, git: repo("main", 2) }),
    P("p11", "legacy-portal", "php", null, { deps: null, git: repo("release/2.4") }),
  ];

  const run = (projectId, projectName, commandId, commandLine, port, startedMinAgo) => ({
    projectId, commandId, projectName, commandName: commandId.replace(/^\w+:/, ""), commandLine,
    cwd: `${root}\\${projectName}`, state: "running", pid: 18000 + port, processCount: 4,
    startedAt: now - startedMinAgo * min, endedAt: null, portHint: port, lastError: null,
  });
  const runs = [
    run("p1", "storefront", "script:dev", "pnpm run dev", 3000, 9),
    run("p2", "billing-api", "script:dev", "pnpm run dev -- --port 8080", 8080, 34),
  ];

  const logs = [
    "> storefront@2.3.0 dev D:\\dev\\storefront",
    "> next dev",
    "",
    "   ▲ Next.js 15.1.0",
    "   - Local:        http://localhost:3000",
    "   - Environments: .env.local",
    "",
    " ✓ Starting...",
    " ✓ Ready in 1.8s",
    " ○ Compiling / ...",
    " ✓ Compiled / in 2.1s (812 modules)",
    " GET / 200 in 2304ms",
    " ○ Compiling /products/[slug] ...",
    " ✓ Compiled /products/[slug] in 640ms (846 modules)",
    " GET /products/walnut-desk 200 in 712ms",
    " GET /api/cart 200 in 38ms",
    " ⚠ Image with src \"/hero.jpg\" has \"fill\" but is missing \"sizes\" prop.",
    " GET /products/oak-shelf 200 in 96ms",
    " GET /api/cart 200 in 21ms",
    " ✓ Compiled /checkout in 410ms (903 modules)",
    " GET /checkout 200 in 455ms",
  ].map((text, seq) => ({ seq, ts: now - (21 - seq) * 20e3, stream: text.startsWith(" ⚠") ? "err" : "out", text }));

  const command = (id, name, commandLine, body, extra = {}) => ({
    id, name, commandLine, cwd: `${root}\\storefront`, source: body ? { kind: "script", body } : { kind: "detected" },
    run: null, knownPort: null, portMode: "default", port: null, portArgs: null, detectedArgs: null, ...extra,
  });

  const settings = {
    editor: "code", terminal: "windowsTerminal", agent: "claude", checkPorts: true, autoPort: true,
    proxy: true, proxyPort: 80, https: true, httpsPort: 443,
  };

  const proc = (pid, name, cmd, cwd) => ({ pid, parent: 4200, name, exe: null, cmd, cwd, startTime: Math.floor((now - 3 * hr) / 1000), session: 1 });
  const portRows = [
    { port: 3000, addresses: ["::1"], pid: 21000, owner: "devDock", process: proc(21000, "node.exe", "node next dev", `${root}\\storefront`), children: [{ pid: 21004, name: "node.exe" }], project: { id: "p1", name: "storefront" }, run: { projectId: "p1", commandId: "script:dev" } },
    { port: 5432, addresses: ["0.0.0.0"], pid: 5120, owner: "external", process: proc(5120, "postgres.exe", "postgres -D C:\\pgdata", "C:\\Program Files\\PostgreSQL\\16\\bin"), children: [], project: null, run: null },
    { port: 5173, addresses: ["127.0.0.1"], pid: 17644, owner: "external", process: proc(17644, "node.exe", "node vite", `${root}\\marketing-site`), children: [], project: { id: "p4", name: "marketing-site" }, run: null },
    { port: 6379, addresses: ["127.0.0.1"], pid: 6020, owner: "external", process: proc(6020, "redis-server.exe", "redis-server", null), children: [], project: null, run: null },
    { port: 8080, addresses: ["0.0.0.0"], pid: 26080, owner: "devDock", process: proc(26080, "node.exe", "node dist/server.js", `${root}\\billing-api`), children: [], project: { id: "p2", name: "billing-api" }, run: { projectId: "p2", commandId: "script:dev" } },
    { port: 135, addresses: ["0.0.0.0"], pid: 1104, owner: "system", process: proc(1104, "svchost.exe", null, null), children: [], project: null, run: null },
    { port: 445, addresses: ["0.0.0.0"], pid: 4, owner: "system", process: proc(4, "System", null, null), children: [], project: null, run: null },
  ];

  const branches = {
    local: [
      { name: "feat/invoices", current: true, upstream: "origin/feat/invoices", ahead: 1, behind: 3, gone: false, updated: "2 hours ago" },
      { name: "main", current: false, upstream: "origin/main", ahead: 0, behind: 0, gone: false, updated: "yesterday" },
      { name: "fix/rounding", current: false, upstream: "origin/fix/rounding", ahead: 0, behind: 0, gone: false, updated: "3 days ago" },
      { name: "spike/webhooks", current: false, upstream: null, ahead: 0, behind: 0, gone: false, updated: "2 weeks ago" },
    ],
    remoteOnly: ["origin/feat/refunds", "origin/chore/deps"],
  };

  const hit = (group, label, hint, action, positions = []) => ({ group, label, hint, action, positions });
  // What typing "api" finds (positions = the matched letters).
  const paletteGroups = [
    { group: "projects", title: "Projects", hits: [
      hit("projects", "billing-api", "D:\\dev\\billing-api", { kind: "openProject", projectId: "p2" }, [8, 9, 10]),
    ] },
    { group: "commands", title: "Commands", hits: [
      hit("commands", "run dev in billing-api", "pnpm run dev", { kind: "runCommand", projectId: "p2", commandId: "script:dev" }, [19, 20, 21]),
      hit("commands", "run test in billing-api", "pnpm run test", { kind: "runCommand", projectId: "p2", commandId: "script:test" }, [20, 21, 22]),
    ] },
    { group: "agents", title: "Agents", hits: [
      hit("agents", "Claude Code in billing-api", "Windows Terminal", { kind: "launchAgent", projectId: "p2", agent: "claude" }, []),
    ] },
    { group: "ports", title: "Ports", hits: [
      hit("ports", "8080 · node.exe", "billing-api · DevDock", { kind: "showPort", port: 8080, pid: 26080 }, []),
    ] },
  ];

  const lib = (q) => {
    let list = projects.slice();
    if (q.group === "-") list = list.filter((p) => !p.group);
    else if (q.group) list = list.filter((p) => p.group === q.group);
    if (q.search) list = list.filter((p) => p.name.includes(q.search.toLowerCase()));
    if (q.sort === "name") list.sort((a, b) => a.name.localeCompare(b.name));
    if (q.sort === "recent") list.sort((a, b) => b.lastOpened - a.lastOpened);
    const g = {};
    for (const p of projects) if (p.group) g[p.group] = (g[p.group] || 0) + 1;
    const kinds = {};
    for (const p of list) kinds[p.kind] = (kinds[p.kind] || 0) + 1;
    return JSON.parse(JSON.stringify({
      projects: list,
      counts: { all: projects.length, favorites: 2, recent: projects.length, missing: 0, new: 0, kinds, groups: g, ungrouped: 1 },
      groups, scanRoots: [{ path: root, depth: 2 }], scanning: false, savedTo: "%APPDATA%\\DevDock\\devdock.json",
    }));
  };

  // A security scan of "storefront": advisory ids, versions and counts are made up.
  const securityReport = () => {
    const V = (id, severity, pkg, version, fixedIn, title, tools, extra = {}) => ({
      key: `vuln:${pkg}@${version}:${id}`, kind: "vulnerability", id, aliases: [], severity, title, package: pkg, version,
      ecosystem: "npm", fixedIn, location: "pnpm-lock.yaml", line: null, commit: null, url: `https://github.com/advisories/${id}`,
      cvss: null, dev: false, remediation: null, tools, isNew: false, ...extra,
    });
    const findings = [
      { key: "secret:stripe-access-token:src/lib/billing.ts:12:", kind: "secret", id: "stripe-access-token", aliases: [], severity: "high",
        title: "Stripe access token", package: null, version: null, ecosystem: null, fixedIn: null, location: "src/lib/billing.ts", line: 12,
        commit: null, url: null, cvss: null, dev: null, remediation: null, tools: ["gitleaks", "trivy"], isNew: true },
      V("GHSA-9w4p-jx7c-q2m5", "high", "next", "15.2.0", "15.2.3", "Authorization bypass in middleware", ["grype", "pnpm"], { cvss: 9.1, aliases: ["CVE-2025-29927"] }),
      V("GHSA-3h52-269p-cp9r", "high", "path-to-regexp", "6.2.1", "6.3.0", "Backtracking regular expressions cause ReDoS", ["grype", "pnpm"], { cvss: 7.5 }),
      V("GHSA-qwph-4952-7xr6", "medium", "next", "15.2.0", "15.2.2", "Cache poisoning of pages with a default locale", ["pnpm"], { cvss: 5.3, isNew: true }),
      V("GHSA-7fh5-64p2-3v2j", "medium", "postcss", "8.4.21", "8.4.31", "Line return parsing error", ["grype"], { cvss: 5.3, dev: true }),
      V("GHSA-pxg6-pf52-xh8x", "low", "cookie", "0.6.0", "0.7.0", "cookie accepts cookie name, path, and domain with out of bounds characters", ["grype", "pnpm"]),
      V("GHSA-c2qf-rxjj-qqgw", "low", "semver", "7.5.1", "7.5.2", "Regular expression denial of service", ["grype", "pnpm"], { dev: true }),
    ];
    findings.push({ key: "config:AVD-DS-0002:Dockerfile:0", kind: "misconfiguration", id: "AVD-DS-0002", aliases: [], severity: "high",
      title: "Image user should not be 'root'", package: null, version: null, ecosystem: null, fixedIn: null, location: "Dockerfile", line: null,
      commit: null, url: "https://avd.aquasec.com/misconfig/ds002", cvss: null, dev: null, remediation: "Add 'USER <non root user name>' line to the Dockerfile",
      tools: ["trivy"], isNew: true });
    const counts = { critical: 0, high: 3, medium: 2, low: 2, unknown: 0, secrets: 1, misconfigs: 1, code: 0 };
    const report = {
      version: 1, projectId: "p1", projectName: "storefront", path: `${root}\\storefront`, startedAt: now - 2 * hr - 41_000,
      finishedAt: now - 2 * hr, options: { history: false },
      tools: [
        { tool: "grype", name: "Grype", status: "ran", detail: null, install: null, command: "grype dir:. -o json -q", findings: 5, ms: 31_400 },
        { tool: "pnpm", name: "pnpm audit", status: "ran", detail: null, install: null, command: "pnpm audit --json", findings: 5, ms: 2_100 },
        { tool: "trivy", name: "Trivy", status: "ran", detail: null, install: null, command: "trivy fs --format json --quiet --scanners vuln,misconfig,secret .", findings: 2, ms: 18_200 },
        { tool: "gitleaks", name: "Gitleaks", status: "ran", detail: null, install: null, command: "gitleaks dir . --redact", findings: 1, ms: 6_800 },
      ],
      findings, counts,
      upgrades: [
        { package: "next", version: "15.2.0", ecosystem: "npm", to: "15.2.3", fixes: 2, unfixed: 0, worst: "high", ids: [], dev: false },
        { package: "path-to-regexp", version: "6.2.1", ecosystem: "npm", to: "6.3.0", fixes: 1, unfixed: 0, worst: "high", ids: [], dev: false },
        { package: "postcss", version: "8.4.21", ecosystem: "npm", to: "8.4.31", fixes: 1, unfixed: 0, worst: "medium", ids: [], dev: true },
        { package: "cookie", version: "0.6.0", ecosystem: "npm", to: "0.7.0", fixes: 1, unfixed: 0, worst: "low", ids: [], dev: false },
        { package: "semver", version: "7.5.1", ecosystem: "npm", to: "7.5.2", fixes: 1, unfixed: 0, worst: "low", ids: [], dev: true },
      ],
      previousAt: now - 6 * day,
      resolved: [{ id: "GHSA-grv7-fg5c-xmjg", kind: "vulnerability", severity: "medium", title: "Uncontrolled resource consumption", package: "braces", version: "3.0.2" }],
      cancelled: false,
    };
    return { report, summary: { finishedAt: report.finishedAt, counts, new: 2 } };
  };

  const scanner = (id, name, covers, install, method, source, extra = {}) => ({
    id, name, covers, install, url: "", path: null, managed: false, method, source, blocked: null, version: null, ...extra,
  });
  const scanners = [
    scanner("grype", "Grype", "Known vulnerabilities in lockfiles of every ecosystem (npm, Cargo, Composer, pip, Go, ...)", "choco install grype   or   scoop install grype", "download", "Downloads the latest release from github.com/anchore/grype and checks its SHA-256", { path: "C:\\tools\\grype.exe" }),
    scanner("trivy", "Trivy", "Vulnerabilities, secrets, and misconfigurations in Dockerfiles, Compose, Kubernetes and Terraform", "choco install trivy   or   scoop install trivy", "download", "Downloads the latest release from github.com/aquasecurity/trivy and checks its SHA-256", { path: "%APPDATA%\\DevDock\\tools\\bin\\trivy.exe", managed: true, version: "v0.60.0" }),
    scanner("osv-scanner", "OSV-Scanner", "Vulnerabilities from the OSV.dev database, across every lockfile in the project", "scoop install osv-scanner", "download", "Downloads the latest release from github.com/google/osv-scanner and checks its SHA-256"),
    scanner("gitleaks", "Gitleaks", "API keys, tokens and passwords committed to the code", "choco install gitleaks   or   scoop install gitleaks", "download", "Downloads the latest release from github.com/gitleaks/gitleaks and checks its SHA-256", { path: "%APPDATA%\\DevDock\\tools\\bin\\gitleaks.exe", managed: true, version: "v8.28.0" }),
    scanner("semgrep", "Semgrep", "Risky patterns in your own code: injection, unsafe eval, weak crypto, ...", "pip install semgrep   (needs Python 3.9+)", "python", "Installs semgrep with pip into a Python environment of its own"),
    scanner("cargo-audit", "cargo audit", "RustSec advisories for Cargo.lock", "cargo install cargo-audit --locked", "cargo", "Builds cargo-audit with cargo install", { blocked: "Needs Rust's cargo: install Rust from rustup.rs, then check again." }),
  ];

  // Sends an app event (e.g. "git:updated") to whoever listens, like Tauri does.
  const emit = (event, payload) => {
    for (const id of listeners.get(event) || []) callbacks.get(id)?.({ event, id, payload });
  };

  const handlers = {
    refresh_projects: () => null,
    git_refresh: (a) => {
      const ids = a.ids && a.ids.length ? a.ids : projects.map((p) => p.id);
      setTimeout(() => ids.forEach((id) => emit("git:updated", { id, details: !!a.details })), 150);
      return null;
    },
    startup_notice: () => null,
    tools_scan: () => ({ tools: [{ id: "explorer", name: "Explorer", path: "C:\\Windows\\explorer.exe" }, { id: "code", name: "VS Code", path: "C:\\Users\\you\\AppData\\Local\\Programs\\Microsoft VS Code\\bin\\code.cmd" }, { id: "git", name: "Git", path: "C:\\Program Files\\Git\\cmd\\git.exe" }], gitBash: "C:\\Program Files\\Git\\git-bash.exe", checkedAt: now }),
    library: (a) => lib(a.query || {}),
    runs_list: () => runs,
    workspaces_list: () => [],
    git_details: (a) => {
      const p = projects.find((x) => x.id === a.id);
      if (!p) return null;
      return { state: "repo", refreshedAt: now, data: {
        root: p.path, isWorktree: false, inSubfolder: false, status: p.git.data,
        lastCommit: { hash: "a41c9e2", subject: "Add invoice PDF export", author: "you", relative: "2 hours ago" },
        remotes: [{ name: "origin", url: `git@github.com:example/${p.name}.git` }],
        worktrees: [{ path: p.path, branch: p.git.data.branch, detached: false, bare: false, isCurrent: true }],
      } };
    },
    git_branches: () => branches,
    git_op: () => ({ ok: true, summary: "Pulled 3 commits", output: "Updating 7d1e0b4..a41c9e2\nFast-forward\n src/invoices/pdf.ts | 48 +++++++++++++\n 3 files changed, 61 insertions(+), 4 deletions(-)" }),
    run_commands: (a) => {
      if (a.projectId !== "p1") return { commands: [], warning: null, host: "billing-api.localhost", hostsMissing: false, proxy: { http: 80, https: 443 }, portless: true, proxyError: null, proxyNote: null };
      return {
        commands: [
          command("script:dev", "dev", "pnpm run dev", "next dev", { run: runs[0], knownPort: 3000, portMode: "auto", port: 3000, detectedArgs: "--port {port}" }),
          command("script:build", "build", "pnpm run build", "next build"),
          command("script:start", "start", "pnpm run start", "next start"),
          command("script:lint", "lint", "pnpm run lint", "next lint"),
          command("script:test", "test", "pnpm run test", "vitest"),
          command("deps:install", "install packages", "pnpm install", null),
        ],
        warning: null, host: "storefront.localhost", hostsMissing: false, proxy: { http: 80, https: 443 },
        portless: true, proxyError: null, proxyNote: null,
      };
    },
    run_logs: (a) => ({ lines: logs.filter((l) => l.seq >= a.since), nextSeq: logs.length, truncated: false }),
    ports_snapshot: () => ({ rows: portRows, refreshedAt: now }),
    agents_overview: () => ({
      agents: [
        { id: "claude", name: "Claude Code", command: "claude", path: "C:\\Users\\you\\AppData\\Roaming\\npm\\claude.cmd", installHint: "npm install -g @anthropic-ai/claude-code" },
        { id: "codex", name: "Codex", command: "codex", path: "C:\\Users\\you\\AppData\\Roaming\\npm\\codex.cmd", installHint: "npm install -g @openai/codex" },
        { id: "opencode", name: "OpenCode", command: "opencode", path: null, installHint: "npm install -g opencode-ai" },
      ],
      terminals: [
        { kind: "windowsTerminal", name: "Windows Terminal", available: true },
        { kind: "powerShell", name: "PowerShell", available: true },
        { kind: "gitBash", name: "Git Bash", available: true },
        { kind: "cmd", name: "Command Prompt", available: true },
      ],
      editors: [
        { command: "code", name: "VS Code", path: "C:\\Users\\you\\AppData\\Local\\Programs\\Microsoft VS Code\\bin\\code.cmd" },
        { command: "cursor", name: "Cursor", path: null },
        { command: "code-insiders", name: "VS Code Insiders", path: null },
        { command: "windsurf", name: "Windsurf", path: null },
        { command: "zed", name: "Zed", path: null },
      ],
      settings, shortcuts: [{ slot: 1, projectId: "p1", agent: "claude", terminal: "windowsTerminal", subdir: null, projectName: "storefront" }],
      tools: handlers.tools_scan(),
    }),
    proxy_status: () => ({ http: 80, https: 443, httpError: null, httpsError: null, httpNote: null, httpsNote: null, ca: { path: "%APPDATA%\\DevDock\\devdock-ca.pem", trusted: true }, hosts: { path: "C:\\Windows\\System32\\drivers\\etc\\hosts", needed: [], missing: [], stale: false, error: null } }),
    palette_open: () => paletteGroups,
    palette_search: () => paletteGroups,
    project_folders: () => ["apps/web", "packages/ui"],
    project_worktrees: () => [],
    tools_recheck: () => handlers.tools_scan(),
    mark_seen: () => false,
    workspace_logs: () => ({ lines: [], cursors: {} }),
    // Files tab: the same small made-up tree for every project.
    files_list: (a) => {
      const dir = a.dir ? `${a.dir}/` : "";
      const names = new Map();
      for (const p of [...Object.keys(demoFiles), ...demoDirs]) {
        if (!p.startsWith(dir)) continue;
        const rest = p.slice(dir.length);
        const name = rest.split("/")[0];
        if (!name) continue;
        const isDir = rest.includes("/") || demoDirs.includes(p);
        names.set(name, (names.get(name) ?? false) || isDir);
      }
      if (!a.showHidden) for (const n of [...names.keys()]) if (n.startsWith(".")) names.delete(n);
      const entries = [...names].map(([name, isDir]) => ({ name, path: dir + name, dir: isDir, ignored: name === "node_modules" || name === ".env.local", hidden: name.startsWith(".") }));
      entries.sort((x, y) => Number(y.dir) - Number(x.dir) || x.name.toLowerCase().localeCompare(y.name.toLowerCase()));
      return { entries, hidden: 0 };
    },
    file_read: (a) => {
      if (!(a.path in demoFiles)) throw { kind: "fileBinary", message: `${a.path} isn't a text file.`, fix: "Images, archives and other binary files open in their own programs." };
      return { text: demoFiles[a.path], format: { eol: "lf", bom: false }, mtime: fileTimes[a.path] ?? now };
    },
    file_stat: (a) => (a.path in demoFiles ? (fileTimes[a.path] ?? now) : null),
    file_write: (a) => {
      demoFiles[a.path] = a.text;
      fileTimes[a.path] = Date.now();
      return fileTimes[a.path];
    },
    files_search: (a) => {
      const q = a.query;
      if (!q.text) return { files: [], total: 0, truncated: false, cancelled: false };
      let src = q.regex ? q.text : q.text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
      if (q.wholeWord) src = `\\b${src}\\b`;
      const re = new RegExp(src, q.matchCase ? "g" : "gi");
      const files = [];
      let total = 0;
      for (const [path, text] of Object.entries(demoFiles)) {
        const matches = [];
        text.split("\n").forEach((line, i) => {
          const ranges = [...line.matchAll(re)].map((m) => [m.index, m.index + m[0].length]);
          if (ranges.length) matches.push({ line: i + 1, text: line, offset: 0, ranges });
        });
        if (matches.length) files.push({ path, matches });
        total += matches.length;
      }
      return { files, total, truncated: false, cancelled: false };
    },
    files_search_cancel: () => null,
    files_set_unsaved: () => null,
    security_scanners: () => scanners.map((x) => ({ ...x })),
    security_installing: () => [],
    security_install: (a) => {
      const s = scanners.find((x) => x.id === a.tool);
      const send = (at, e) => setTimeout(() => emit("security:install", { tool: a.tool, state: "running", stage: "", progress: null, line: null, error: null, ...e }), at);
      let t = 150;
      const step = (stage) => send((t += 400), { stage, line: `== ${stage}` });
      const line = (text, stage) => send((t += 120), { stage, line: text });
      if (s.method === "python") {
        step("Creating a Python environment");
        line(`> "C:\\Program Files\\Python313\\py.exe" -m venv "%APPDATA%\\DevDock\\tools\\${s.id}"`, "Creating a Python environment");
        const st = `Installing ${s.id} with pip (a few minutes)`;
        step(st);
        line(`> "%APPDATA%\\DevDock\\tools\\${s.id}\\Scripts\\python.exe" -m pip install --upgrade ${s.id}`, st);
        ["Collecting semgrep", "  Downloading semgrep-1.140.0-cp39.cp310.cp311.py37.py38.py39.py310.py311-none-win_amd64.whl (48.1 MB)", "Collecting attrs>=21.3", "Collecting boltons~=21.0", "Collecting click-option-group~=0.5", "ERROR: Could not find a version that satisfies the requirement semgrep-core (from semgrep)", "ERROR: No matching distribution found for semgrep-core"].forEach((l) => line(l, st));
        setTimeout(() => emit("security:install", { tool: a.tool, state: "failed", stage: "Failed", progress: null, line: "== Failed: pip install semgrep failed: ERROR: No matching distribution found for semgrep-core", error: "pip install semgrep failed: ERROR: No matching distribution found for semgrep-core" }), (t += 300));
        return null;
      }
      step("Finding the latest release");
      line(`> "C:\\WINDOWS\\System32\\curl.exe" -fsSL --retry 2 https://api.github.com/repos/demo/${s.id}/releases/latest`, "Finding the latest release");
      const dl = `Downloading ${s.name} v1.0.0 (38 MB)`;
      step(dl);
      line(`> "C:\\WINDOWS\\System32\\curl.exe" -fL --retry 2 -s -o "${s.id}_1.0.0_windows_amd64.zip" "https://github.com/demo/${s.id}/releases/download/v1.0.0/${s.id}_1.0.0_windows_amd64.zip"`, dl);
      for (let p = 0; p <= 100; p += 4) send((t += 90), { stage: dl, progress: p / 100 });
      line(`Downloaded ${s.id}_1.0.0_windows_amd64.zip (39845888 bytes)`, dl);
      step("Checking the download");
      line("SHA-256 3f7c1e0b9a64d2c58e17a0f4b6d93c2e85a1f07d4c6b2e9a8d5f3c1b0e7a9d42", "Checking the download");
      line("expected 3f7c1e0b9a64d2c58e17a0f4b6d93c2e85a1f07d4c6b2e9a8d5f3c1b0e7a9d42", "Checking the download");
      step("Unpacking");
      line(`> "C:\\WINDOWS\\System32\\tar.exe" -xf "${s.id}_1.0.0_windows_amd64.zip" -C "x"`, "Unpacking");
      const path = `%APPDATA%\\DevDock\\tools\\bin\\${s.id}.exe`;
      line(`Installed ${path}`, "Unpacking");
      setTimeout(() => {
        Object.assign(s, { path, managed: true, version: "v1.0.0" });
        emit("security:install", { tool: a.tool, state: "done", stage: "Installed", progress: null, line: `== Done: ${path}`, error: null });
      }, (t += 300));
      return null;
    },
    security_install_log: () => null,
    security_install_cancel: () => null,
    security_uninstall: (a) => {
      Object.assign(scanners.find((x) => x.id === a.tool), { path: null, managed: false, version: null });
      return null;
    },
    security_running: () => [],
    security_summaries: () => ({
      p1: securityReport().summary,
      p3: { finishedAt: now - 3 * day, counts: { critical: 1, high: 2, medium: 4, low: 1, unknown: 0, secrets: 0 }, new: 0 },
    }),
    security_report: (a) => (a.id === "p1" ? securityReport().report : null),
    security_scan: (a) => {
      const steps = ["Grype", "pnpm audit", "Gitleaks"];
      steps.forEach((name, i) =>
        setTimeout(() => emit("security:progress", { projectId: a.id, step: i + 1, total: steps.length, tool: name.split(" ")[0].toLowerCase(), name, startedAt: Date.now() }), 300 + i * 1200),
      );
      setTimeout(() => emit("security:done", { projectId: a.id, summary: securityReport().summary, error: null }), 300 + steps.length * 1200);
      return null;
    },
    security_cancel: () => null,
    security_export: () => null,
    security_open_advisory: () => null,
    security_open_file: () => null,
    "plugin:app|version": () => "1.1.0",
    "plugin:app|name": () => "DevDock",
    "plugin:updater|check": () => null,
  };

  const demoDirs = ["node_modules", "public"];
  const fileTimes = {};
  const demoFiles = {
    "package.json": `{\n  "name": "storefront",\n  "private": true,\n  "scripts": {\n    "dev": "next dev",\n    "build": "next build",\n    "test": "vitest"\n  },\n  "dependencies": {\n    "next": "15.2.0",\n    "react": "19.0.0"\n  }\n}\n`,
    "README.md": `# storefront\n\nThe shop front end. Run \`pnpm dev\` and open http://storefront.localhost.\n\n## Checkout\n\nThe cart lives in \`src/lib/cart.ts\`.\n`,
    ".env.local": `NEXT_PUBLIC_API_URL=http://billing-api.localhost\n`,
    "src/app/page.tsx": `import { formatPrice } from "../lib/cart";\nimport { ProductCard } from "../components/ProductCard";\n\nexport default async function Home() {\n  const products = await fetch("/api/products").then((r) => r.json());\n  return (\n    <main className="grid">\n      {products.map((p) => (\n        <ProductCard key={p.id} product={p} price={formatPrice(p.cents)} />\n      ))}\n    </main>\n  );\n}\n`,
    "src/lib/cart.ts": `export interface CartItem {\n  id: string;\n  quantity: number;\n  cents: number;\n}\n\n/** Total in cents, before tax. */\nexport function cartTotal(items: CartItem[]): number {\n  return items.reduce((sum, item) => sum + item.cents * item.quantity, 0);\n}\n\nexport function formatPrice(cents: number): string {\n  return new Intl.NumberFormat("en-US", { style: "currency", currency: "USD" }).format(cents / 100);\n}\n`,
    "src/components/ProductCard.tsx": `import { formatPrice } from "../lib/cart";\n\nexport function ProductCard({ product, price }) {\n  return (\n    <article className="card">\n      <h2>{product.name}</h2>\n      <p>{price ?? formatPrice(product.cents)}</p>\n    </article>\n  );\n}\n`,
  };

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
