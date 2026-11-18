<script lang="ts">
  import { app } from "$lib/app.svelte";
  import * as api from "$lib/api";
  import Button from "$lib/components/Button.svelte";
  import Checkbox from "$lib/components/Checkbox.svelte";
  import ErrorBanner from "$lib/components/ErrorBanner.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import InstallLogDialog from "./InstallLogDialog.svelte";
  import { duration, plural, relativeTime } from "$lib/format";
  import { files } from "$lib/files.svelte";
  import { runs } from "$lib/runs.svelte";
  import { SEVERITIES, SEVERITY_LABEL, security } from "$lib/security.svelte";
  import type { Finding, Project, Severity, ToolRun } from "$lib/types";

  // Runs the security scanners installed on this machine (Grype, the package
  // manager's audit, cargo audit, Gitleaks), shows their merged findings, and
  // exports them as an HTML report. DevDock scans nothing itself.
  let { project }: { project: Project } = $props();

  const report = $derived(security.reports[project.id]);
  const active = $derived(security.running[project.id]);
  const loaded = $derived(project.id in security.reports);
  const error = $derived(security.errors[project.id]);
  let exported = $state<string | null>(null);

  $effect(() => {
    if (!loaded && !security.loading[project.id]) security.load(project.id);
    security.loadScanners();
  });

  // Filters for the findings list.
  type Filter = "all" | Severity | "secret" | "misconfiguration" | "code" | "new";
  let filter = $state<Filter>("all");
  let query = $state("");

  const toolNames = $derived(new Map((report?.tools ?? []).map((t) => [t.tool, t.name])));
  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return (report?.findings ?? []).filter((f) => {
      if ((filter === "secret" || filter === "misconfiguration" || filter === "code") && f.kind !== filter) return false;
      if (filter === "new" && !f.isNew) return false;
      if (SEVERITIES.includes(filter as Severity) && (f.kind === "secret" || f.severity !== filter)) return false;
      if (!q) return true;
      return [f.id, f.title, f.package, f.location, ...f.aliases].some((x) => x?.toLowerCase().includes(q));
    });
  });
  const didRun = (...tools: string[]) => report?.tools.some((t) => tools.includes(t.tool) && t.status === "ran") ?? false;
  const secretsScanned = $derived(didRun("gitleaks", "trivy"));
  const configScanned = $derived(didRun("trivy"));
  const codeScanned = $derived(didRun("semgrep"));
  const newCount = $derived(report?.findings.filter((f) => f.isNew).length ?? 0);
  const ran = $derived(report?.tools.filter((t) => t.status === "ran").length ?? 0);
  // Package audits and other runs that aren't in the installable scanner list.
  const extraRuns = $derived((report?.tools ?? []).filter((t) => !security.scanners?.some((s) => s.id === t.tool)));
  const lastRun = (id: string) => report?.tools.find((t) => t.tool === id);

  const verdict = $derived.by(() => {
    if (!report) return null;
    const c = report.counts;
    const vulns = c.critical + c.high + c.medium + c.low + c.unknown;
    if (ran === 0) return { tone: "warn", text: "No scanner ran. Install one below and scan again." };
    if (c.secrets) return { tone: "bad", text: `${plural(c.secrets, "possible secret")} in the code. Rotate any real credential first.` };
    if (c.critical + c.high)
      return { tone: "bad", text: `${plural(c.critical + c.high, "critical or high-severity issue")} to look at.` };
    if (vulns) return { tone: "warn", text: `${plural(vulns, "known issue")}, none critical or high.` };
    return { tone: "ok", text: secretsScanned ? "No known vulnerabilities or secrets." : "No known vulnerabilities." };
  });

  const firstRun = $derived(!report && !active);
  const dbFirst = $derived(
    !report && (active?.progress?.tool === "grype" || active?.progress?.tool === "trivy") ? active.progress.name : null,
  );

  async function exportReport() {
    exported = await security.export(project.id, project.name);
  }

  function openFinding(f: Finding) {
    api.securityOpenAdvisory(project.id, f.key).catch((err) => (security.errors[project.id] = api.toErrorDto(err)));
  }

  function openFile(f: Finding) {
    if (!f.location) return;
    app.tab = "files";
    files.open(project.id, f.location, f.line ? { line: f.line, from: 0, to: 0 } : null);
  }

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      // Clipboard blocked: nothing else to do.
    }
  }

  function toolState(t: ToolRun): { tone: string; label: string } {
    switch (t.status) {
      case "ran":
        return { tone: "ok", label: t.findings ? plural(t.findings, "finding") : "Clean" };
      case "failed":
        return { tone: "bad", label: "Failed" };
      case "cancelled":
        return { tone: "muted", label: "Stopped" };
      default:
        return { tone: "muted", label: "Skipped" };
    }
  }

  const FILTERS: { key: Filter; label: string }[] = [
    { key: "all", label: "All" },
    { key: "critical", label: "Critical" },
    { key: "high", label: "High" },
    { key: "medium", label: "Medium" },
    { key: "low", label: "Low" },
    { key: "secret", label: "Secrets" },
    { key: "misconfiguration", label: "Config" },
    { key: "code", label: "Code" },
    { key: "new", label: "New" },
  ];
  function count(key: Filter): number {
    if (!report) return 0;
    if (key === "all") return report.findings.length;
    if (key === "secret") return report.counts.secrets;
    if (key === "misconfiguration") return report.counts.misconfigs ?? 0;
    if (key === "code") return report.counts.code ?? 0;
    if (key === "new") return newCount;
    return report.counts[key];
  }
</script>

<section class="panel scan" aria-label="Security scan">
  <div class="top">
    <span class="eyebrow">Security scan</span>
    {#if active}
      <span class="pill busy"><span class="spinner" aria-hidden="true"></span>Scanning…</span>
    {:else if report}
      <span class="when">
        Last scan {relativeTime(report.finishedAt, runs.now)} · took {duration(report.finishedAt - report.startedAt)}
      </span>
    {:else if loaded}
      <span class="when">Never scanned</span>
    {/if}
    <span class="spacer"></span>
    {#if !active}
      <label class="opt" title="Gitleaks also reads every commit, which is slower on large repositories">
        <Checkbox
          checked={security.history[project.id] ?? false}
          onchange={(v) => (security.history[project.id] = v)}
          disabled={!project.isGit}
        />
        Include git history
      </label>
    {/if}
    {#if report && !active}
      <Button variant="secondary" onclick={exportReport} title="Save this report as an HTML file you can share or print">
        <Icon name="download" size={14} /> Export report
      </Button>
    {/if}
    {#if active}
      <button class="stop" disabled={security.stopping[project.id]} onclick={() => security.cancel(project.id)}>
        ■ {security.stopping[project.id] ? "Stopping…" : "Stop"}
      </button>
    {:else}
      <Button variant="primary" onclick={() => security.scan(project.id)} disabled={project.missing}>
        <Icon name="shield" size={14} />
        {report ? "Scan again" : "Scan project"}
      </Button>
    {/if}
  </div>

  {#if active}
    <div class="progress" role="status">
      {#if active.progress}
        Running <strong>{active.progress.name}</strong>
        <span class="muted">({active.progress.step} of {active.progress.total})</span>
      {:else}
        Starting…
      {/if}
      <span class="mono muted">{duration(runs.now - active.startedAt)}</span>
    </div>
    {#if dbFirst}
      <p class="note">
        {dbFirst} downloads its vulnerability database (a few hundred MB) the first time it runs, which can take a few
        minutes.
      </p>
    {/if}
  {:else if firstRun}
    <p class="text">
      Runs the scanners installed on this machine against <span class="mono">{project.name}</span>, merges what they
      find, and keeps the report so the next scan can show what changed.
    </p>
  {/if}
  <p class="note">
    <Icon name="lock" size={12} /> Your code stays on this machine. Package audits and OSV-Scanner send the list of
    dependencies to their advisory databases, Grype and Trivy download theirs, and Semgrep downloads its rules (with
    metrics off).
  </p>
  {#if exported}
    <p class="note ok"><Icon name="check" size={12} /> Saved to <span class="mono">{exported}</span></p>
  {/if}
</section>

{#if error}<ErrorBanner {error} ondismiss={() => (security.errors[project.id] = null)} />{/if}

{#if report}
  {#if report.cancelled}
    <div class="banner warn"><Icon name="alert" size={14} /> This scan was stopped early, so it's incomplete.</div>
  {/if}

  <section class="panel summary" aria-label="Summary">
    {#if verdict}<div class="verdict {verdict.tone}">{verdict.text}</div>{/if}
    <div class="cards">
      {#each SEVERITIES as s (s)}
        {#if s !== "unknown" || report.counts.unknown}
          <button
            class="card {s}"
            class:zero={!report.counts[s]}
            class:on={filter === s}
            onclick={() => (filter = filter === s ? "all" : s)}
          >
            <span class="n">{report.counts[s]}</span><span class="l">{SEVERITY_LABEL[s]}</span>
          </button>
        {/if}
      {/each}
      <button
        class="card secret"
        class:zero={!report.counts.secrets}
        class:on={filter === "secret"}
        disabled={!secretsScanned}
        title={secretsScanned ? "" : "Gitleaks didn't run, so secrets weren't checked"}
        onclick={() => (filter = filter === "secret" ? "all" : "secret")}
      >
        <span class="n">{secretsScanned ? report.counts.secrets : "–"}</span><span class="l">Secrets</span>
      </button>
      {#if configScanned || report.counts.misconfigs}
        <button
          class="card kind"
          class:zero={!report.counts.misconfigs}
          class:on={filter === "misconfiguration"}
          title="Risky settings in Dockerfiles, Compose, Kubernetes and Terraform (Trivy)"
          onclick={() => (filter = filter === "misconfiguration" ? "all" : "misconfiguration")}
        >
          <span class="n">{report.counts.misconfigs ?? 0}</span><span class="l">Config</span>
        </button>
      {/if}
      {#if codeScanned || report.counts.code}
        <button
          class="card kind"
          class:zero={!report.counts.code}
          class:on={filter === "code"}
          title="Risky patterns in the code (Semgrep)"
          onclick={() => (filter = filter === "code" ? "all" : "code")}
        >
          <span class="n">{report.counts.code ?? 0}</span><span class="l">Code</span>
        </button>
      {/if}
    </div>
    {#if report.counts.misconfigs || report.counts.code}
      <p class="delta">Severity counts include {plural(report.counts.misconfigs ?? 0, "misconfiguration")} and {plural(report.counts.code ?? 0, "code issue")}.</p>
    {/if}
    {#if report.previousAt}
      <p class="delta">
        Since the scan {relativeTime(report.previousAt, runs.now)}:
        <span class:bad={newCount > 0}>{newCount} new</span> ·
        <span class:good={report.resolved.length > 0}>{report.resolved.length} resolved</span>
        {#if report.resolved.length}
          <span class="muted">
            ({report.resolved
              .slice(0, 3)
              .map((r) => (r.package ? `${r.package} ${r.id}` : r.id))
              .join(", ")}{report.resolved.length > 3 ? ", …" : ""})
          </span>
        {/if}
      </p>
    {/if}
  </section>

  {#if report.upgrades.length}
    <section class="panel" aria-label="Recommended upgrades">
      <div class="head">
        <h2>Recommended upgrades</h2>
        <span class="muted small">Worst first. A package you don't depend on directly is upgraded through its parent, or pinned with overrides.</span>
      </div>
      <div class="table">
        {#each report.upgrades as u (u.package + "@" + u.version)}
          <div class="urow">
            <span class="sev {u.worst}">{SEVERITY_LABEL[u.worst]}</span>
            <span class="pkg mono">{u.package}</span>
            <span class="ver mono">
              {u.version ?? "?"}
              {#if u.to}<span class="arrow">→</span> <span class="to">{u.to}</span>{:else}<span class="muted"> · no fix yet</span>{/if}
            </span>
            <span class="muted small">
              {plural(u.fixes + u.unfixed, "issue")}{u.dev ? " · dev only" : ""}
            </span>
            {#if u.to}
              <button class="icon-btn" title="Copy {u.package}@{u.to}" aria-label="Copy {u.package}@{u.to}" onclick={() => copy(`${u.package}@${u.to}`)}>
                <Icon name="copy" size={13} />
              </button>
            {:else}<span></span>{/if}
          </div>
        {/each}
      </div>
    </section>
  {/if}

  {#if report.findings.length}
    <section class="panel" aria-label="Findings">
      <div class="head">
        <h2>Findings <span class="count">{report.findings.length}</span></h2>
        <span class="spacer"></span>
        <label class="search">
          <Icon name="search" size={13} />
          <input placeholder="Filter by id, package or file" bind:value={query} aria-label="Filter findings" />
        </label>
      </div>
      <div class="filters" role="tablist">
        {#each FILTERS as f (f.key)}
          {#if f.key === "all" || count(f.key) > 0}
            <button role="tab" aria-selected={filter === f.key} class:on={filter === f.key} onclick={() => (filter = f.key)}>
              {f.label} <span class="mono">{count(f.key)}</span>
            </button>
          {/if}
        {/each}
      </div>
      <ul class="findings">
        {#each shown as f (f.key)}
          <li>
            {#if f.kind === "secret"}
              <span class="sev secret">Secret</span>
            {:else if f.kind !== "vulnerability"}
              <span class="sev {f.severity}" title={f.kind === "code" ? "Code issue" : "Misconfiguration"}
                >{SEVERITY_LABEL[f.severity]}</span
              >
            {:else}
              <span class="sev {f.severity}">{SEVERITY_LABEL[f.severity]}</span>
            {/if}
            <div class="body">
              <div class="title">
                {f.title || "(no description)"}
                {#if f.isNew}<span class="new">New</span>{/if}
              </div>
              <div class="meta">
                {#if f.kind === "secret"}
                  <span class="mono">{f.id}</span>
                  {#if f.location}
                    · {#if !f.commit}<button class="link mono" onclick={() => openFile(f)} title="Open in the Files tab"
                        >{f.location}{f.line ? `:${f.line}` : ""}</button
                      >{:else}<span class="mono">{f.location}{f.line ? `:${f.line}` : ""}</span> in commit
                      <span class="mono">{f.commit.slice(0, 8)}</span>{/if}
                  {/if}
                {:else if f.kind !== "vulnerability"}
                  <span class="kind-tag">{f.kind === "code" ? "Code" : "Config"}</span>
                  {#if f.url}
                    <button class="link mono" onclick={() => openFinding(f)} title="Open the rule's page">{f.id}</button>
                  {:else}<span class="mono">{f.id}</span>{/if}
                  {#if f.location}
                    · <button class="link mono" onclick={() => openFile(f)} title="Open in the Files tab"
                      >{f.location}{f.line ? `:${f.line}` : ""}</button
                    >
                  {/if}
                  {#if f.remediation}<div class="fix">{f.remediation}</div>{/if}
                {:else}
                  {#if f.url}
                    <button class="link mono" onclick={() => openFinding(f)} title="Open the advisory">{f.id}</button>
                  {:else}<span class="mono">{f.id}</span>{/if}
                  · <span class="mono">{f.package}@{f.version ?? "?"}</span>
                  {#if f.fixedIn}· fixed in <span class="mono to">{f.fixedIn}</span>{:else}· <span>no fix yet</span>{/if}
                  {#if f.cvss}· CVSS {f.cvss.toFixed(1)}{/if}
                  {#if f.dev}· dev only{/if}
                  {#if f.location}· <span class="mono">{f.location}</span>{/if}
                {/if}
              </div>
            </div>
            <span class="tools small muted">{f.tools.map((t) => toolNames.get(t) ?? t).join(", ")}</span>
          </li>
        {:else}
          <li class="empty muted">Nothing matches this filter.</li>
        {/each}
      </ul>
    </section>
  {/if}
{/if}

<section class="panel" aria-label="Scanners">
  <div class="head">
    <h2>Scanners</h2>
    <span class="muted small">Installed ones run on every scan. Install puts a scanner in DevDock's own tools folder.</span>
    <span class="spacer"></span>
    <button class="link small" onclick={() => security.loadScanners(true)}>Check again</button>
  </div>
  <div class="table">
    {#each security.scanners ?? [] as s (s.id)}
      {@const run = lastRun(s.id)}
      {@const st = run ? toolState(run) : null}
      {@const busy = security.installing[s.id]}
      {@const note = security.installNote[s.id]}
      <div class="srow">
        <div class="sname">
          <span class="tname">{s.name}</span>
          {#if busy}
            <span class="st busy"><span class="spinner" aria-hidden="true"></span>Installing</span>
          {:else if s.path}
            <span class="st ok" title={s.path}>Installed{s.version ? ` ${s.version}` : ""}{s.managed ? " · by DevDock" : ""}</span>
          {:else}
            <span class="st muted">Not installed</span>
          {/if}
        </div>
        <div class="tdetail small">
          <div class="muted">{s.covers}</div>
          {#if busy}
            {@const p = security.installProgress[s.id]}
            <div class="istage">
              <span>{busy}…</span>
              {#if p != null}<span class="mono muted">{Math.round(p * 100)}%</span>{/if}
              <button class="link small" onclick={() => security.openLog(s.id)}>Show log</button>
            </div>
            <div class="iprogress" role="progressbar" aria-label="Installing {s.name}" aria-valuemin="0" aria-valuemax="100" aria-valuenow={p != null ? Math.round(p * 100) : undefined}>
              <div class="ibar" class:indeterminate={p == null} style="width: {p != null ? p * 100 : 30}%"></div>
            </div>
          {:else if run && run.status !== "skipped"}
            <div>
              Last scan: <span class="st {st?.tone}">{st?.label}</span>{run.ms ? ` in ${duration(run.ms)}` : ""}{run.detail
                ? ` · ${run.detail}`
                : ""}
            </div>
          {/if}
          {#if note}
            <div class="inote {note.tone}">
              {note.text}
              {#if security.installLogs[s.id]}
                <button class="link small" onclick={() => security.openLog(s.id)}>View log</button>
              {/if}
            </div>
          {/if}
          {#if !s.path && !busy}
            {#if s.blocked}
              <div class="muted">{s.blocked}</div>
            {/if}
            <div class="install">
              <span class="muted">Or by hand:</span>
              <span class="mono">{s.install}</span>
              <button class="icon-btn" aria-label="Copy install command" title="Copy" onclick={() => copy(s.install)}>
                <Icon name="copy" size={12} />
              </button>
            </div>
          {/if}
        </div>
        <div class="sact">
          {#if busy}
            <button class="stop small-btn" onclick={() => security.cancelInstall(s.id)}>Cancel</button>
          {:else if !s.path}
            <Button variant="secondary" onclick={() => security.install(s.id)} disabled={!!s.blocked} title={s.blocked ?? s.source}>
              <Icon name="download" size={14} /> Install
            </Button>
          {:else if s.managed}
            <button class="link small" onclick={() => security.uninstall(s.id)} title="Delete DevDock's copy of {s.name}">Remove</button>
          {/if}
        </div>
      </div>
    {/each}
    {#each extraRuns as t (t.tool)}
      {@const st = toolState(t)}
      <div class="srow">
        <div class="sname">
          <span class="tname">{t.name}</span>
          <span class="st {st.tone}">{st.label}</span>
        </div>
        <div class="tdetail small">
          <div class="muted">The package manager's own advisories.{t.ms ? ` Took ${duration(t.ms)}.` : ""}</div>
          {#if t.detail}<div>{t.detail}</div>{/if}
        </div>
        <div class="sact"></div>
      </div>
    {:else}
      {#if !report}
        <div class="srow">
          <div class="sname"><span class="tname">Package audit</span><span class="st muted">Built in</span></div>
          <div class="tdetail small muted">npm, pnpm, yarn or bun audit, for Node projects with a lockfile.</div>
          <div class="sact"></div>
        </div>
      {/if}
    {/each}
  </div>
</section>

{#if security.logOpen}<InstallLogDialog />{/if}

<style>
  .panel {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    padding: var(--space-7) var(--space-8);
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
  }
  .top,
  .head {
    display: flex;
    align-items: center;
    gap: var(--space-5);
    flex-wrap: wrap;
  }
  .eyebrow {
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .count {
    margin-left: var(--space-2);
    font-size: 12.5px;
    font-weight: 400;
    color: var(--text-muted);
  }
  .spacer {
    flex: 1;
  }
  .when,
  .muted {
    color: var(--text-muted);
  }
  .when {
    font-size: 12.5px;
  }
  .small {
    font-size: 12px;
  }
  .text,
  .note,
  .progress,
  .delta {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-muted);
  }
  .text {
    color: var(--text-soft);
  }
  .note {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  .note.ok {
    color: var(--success);
  }
  .progress {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    color: var(--text);
  }
  .opt {
    display: inline-flex;
    align-items: center;
    gap: var(--space-3);
    font-size: 12.5px;
    color: var(--text-soft);
    cursor: pointer;
  }
  .pill {
    display: inline-flex;
    align-items: center;
    gap: var(--space-3);
    padding: 2px 9px;
    border-radius: var(--radius-pill);
    font-size: 12.5px;
    font-weight: 600;
  }
  .pill.busy {
    background: var(--bg-hover);
  }
  .spinner {
    width: 10px;
    height: 10px;
    border: 1.5px solid var(--line-strong);
    border-top-color: var(--accent-fg);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .stop {
    height: 36px;
    padding: 0 var(--space-8);
    border: 1px solid var(--error-border);
    border-radius: var(--radius-button);
    background: var(--error-pill);
    color: var(--error-text);
    font-weight: 500;
    cursor: pointer;
  }
  .stop:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .banner {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-5) var(--space-7);
    border-radius: var(--radius-panel);
    font-size: 13px;
  }
  .banner.warn {
    background: var(--warning-panel);
    border: 1px solid var(--warning-border);
  }

  /* Summary */
  .verdict {
    font-size: 15px;
    font-weight: 600;
  }
  .verdict.bad {
    color: var(--error-text);
  }
  .verdict.warn {
    color: var(--accent-fg);
  }
  .verdict.ok {
    color: var(--success);
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(96px, 1fr));
    gap: var(--space-4);
  }
  .card {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    padding: var(--space-5) var(--space-6);
    border: 1px solid transparent;
    border-radius: var(--radius-row);
    background: var(--bg-hover);
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .card:disabled {
    cursor: default;
  }
  .card .n {
    font-size: 22px;
    font-weight: 700;
    line-height: 1.15;
    font-variant-numeric: tabular-nums;
  }
  .card .l {
    font-size: 12px;
    color: var(--text-muted);
  }
  .card.critical:not(.zero),
  .card.secret:not(.zero) {
    background: var(--error-pill);
    color: var(--error);
  }
  .card.high:not(.zero) {
    background: var(--error-row);
    color: var(--error-text);
  }
  .card.medium:not(.zero) {
    background: var(--accent-tint);
    color: var(--accent-fg);
  }
  .card.low:not(.zero) {
    color: var(--info);
  }
  .card.zero .n {
    color: var(--text-faint);
  }
  .card.on {
    border-color: var(--line-strong);
    box-shadow: inset 0 0 0 1px var(--line-strong);
  }
  .delta .bad {
    color: var(--error-text);
    font-weight: 600;
  }
  .delta .good {
    color: var(--success);
    font-weight: 600;
  }

  /* Severity pills */
  .sev {
    display: inline-block;
    min-width: 62px;
    padding: 1px 8px;
    border-radius: var(--radius-pill);
    font-size: 11.5px;
    font-weight: 600;
    text-align: center;
    white-space: nowrap;
    background: var(--bg-hover);
    color: var(--text-muted);
  }
  .sev.critical,
  .sev.secret {
    background: var(--error);
    color: var(--error-ink);
  }
  .sev.high {
    background: var(--error-pill);
    color: var(--error-text);
  }
  .sev.medium {
    background: var(--accent-tint);
    color: var(--accent-fg);
  }
  .sev.low {
    color: var(--info);
  }

  /* Tables */
  .table {
    display: flex;
    flex-direction: column;
  }
  .urow {
    display: grid;
    align-items: center;
    gap: var(--space-5);
    padding: var(--space-4) 0;
    border-top: 1px solid var(--line);
    font-size: 13px;
  }
  .urow:first-child {
    border-top: 0;
  }
  .urow {
    grid-template-columns: auto minmax(140px, 240px) 1fr auto 28px;
  }
  .urow .ver {
    justify-self: start;
    text-align: left;
  }
  .srow {
    display: grid;
    grid-template-columns: 170px 1fr auto;
    align-items: start;
    gap: var(--space-6);
    padding: var(--space-5) 0;
    border-top: 1px solid var(--line);
    font-size: 13px;
  }
  .srow:first-child {
    border-top: 0;
  }
  .sname {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .sact {
    display: flex;
    align-items: center;
    min-height: 36px;
  }
  .st.busy {
    display: inline-flex;
    align-items: center;
    gap: var(--space-3);
    color: var(--text);
    font-weight: 400;
  }
  .small-btn {
    height: 30px;
    padding: 0 var(--space-6);
  }
  .istage {
    display: flex;
    align-items: baseline;
    gap: var(--space-4);
    color: var(--text);
  }
  .iprogress {
    height: 4px;
    margin-top: var(--space-2);
    overflow: hidden;
    background: var(--bg-hover);
    border-radius: var(--radius-pill);
  }
  .ibar {
    height: 100%;
    background: var(--accent);
    border-radius: inherit;
    transition: width 200ms var(--ease);
  }
  .ibar.indeterminate {
    width: 30% !important;
    animation: islide 1.1s var(--ease) infinite;
  }
  @keyframes islide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(340%);
    }
  }
  .inote {
    display: flex;
    align-items: baseline;
    gap: var(--space-4);
  }
  .inote.ok {
    color: var(--success);
  }
  .inote.bad {
    color: var(--error-text);
  }
  .kind-tag {
    display: inline-block;
    margin-right: var(--space-2);
    padding: 0 6px;
    border: 1px solid var(--line);
    border-radius: var(--radius-badge);
    font-size: 11px;
    color: var(--text-soft);
  }
  .fix {
    margin-top: var(--space-1);
    color: var(--text-soft);
  }
  .card.kind:not(.zero) {
    background: var(--accent-tint);
    color: var(--accent-fg);
  }
  .pkg {
    overflow-wrap: anywhere;
  }
  .arrow {
    color: var(--text-faint);
  }
  .to {
    color: var(--success);
    font-weight: 600;
  }
  .tname {
    font-weight: 600;
  }
  .st {
    font-size: 12.5px;
    font-weight: 600;
  }
  .st.ok {
    color: var(--success);
  }
  .st.bad {
    color: var(--error-text);
  }
  .st.muted {
    color: var(--text-muted);
    font-weight: 400;
  }
  .tdetail {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
    color: var(--text-soft);
  }
  .install {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  .cmd {
    font-size: 11.5px;
    overflow-wrap: anywhere;
  }
  .icon-btn {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--icon);
    cursor: pointer;
  }
  .icon-btn:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
  .link {
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--accent-fg);
    font: inherit;
    cursor: pointer;
  }
  .link:hover {
    text-decoration: underline;
  }

  /* Findings */
  .search {
    display: inline-flex;
    align-items: center;
    gap: var(--space-3);
    height: 30px;
    padding: 0 var(--space-5);
    border: 1px solid var(--line);
    border-radius: var(--radius-button);
    color: var(--icon);
  }
  .search input {
    width: 220px;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 13px;
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-3);
  }
  .filters button {
    height: 26px;
    padding: 0 var(--space-5);
    border: 1px solid var(--line);
    border-radius: var(--radius-pill);
    background: transparent;
    color: var(--text-soft);
    font: inherit;
    font-size: 12.5px;
    cursor: pointer;
  }
  .filters button .mono {
    margin-left: var(--space-1);
    color: var(--text-faint);
    font-size: 11.5px;
  }
  .filters button.on {
    background: var(--accent-tint);
    border-color: var(--accent-panel-border);
    color: var(--text);
  }
  .findings {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .findings li {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: start;
    gap: var(--space-5);
    padding: var(--space-5) 0;
    border-top: 1px solid var(--line);
  }
  .findings li:first-child {
    border-top: 0;
  }
  .findings li.empty {
    display: block;
    font-size: 13px;
  }
  .body {
    min-width: 0;
  }
  .title {
    font-size: 13.5px;
    color: var(--text);
  }
  .meta {
    margin-top: var(--space-1);
    font-size: 12px;
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }
  .meta .mono,
  .meta .link {
    font-size: 11.5px;
  }
  .new {
    display: inline-block;
    margin-left: var(--space-3);
    padding: 0 6px;
    border-radius: var(--radius-badge);
    background: var(--accent);
    color: var(--accent-ink);
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    vertical-align: 1px;
  }
  .tools {
    text-align: right;
    max-width: 140px;
  }
</style>
