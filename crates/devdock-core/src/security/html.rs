//! The exported report: one self-contained HTML file (inline CSS, no network),
//! readable in light and dark mode and printable to PDF from any browser.

use std::fmt::Write as _;

use super::merge::Upgrade;
use super::model::{Finding, FindingKind, Report, Severity, ToolStatus};

fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Only http(s) links are written as links.
fn safe_url(url: &str) -> Option<String> {
    (url.starts_with("https://") || url.starts_with("http://")).then(|| esc(url))
}

/// `2026-11-18 14:05 UTC` from epoch milliseconds (no date crate needed).
pub fn utc(ms: u64) -> String {
    let secs = ms / 1000;
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02} UTC",
        rem / 3600,
        (rem % 3600) / 60
    )
}

fn duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms} ms")
    } else if ms < 60_000 {
        format!("{:.1} s", ms as f64 / 1000.0)
    } else {
        format!("{} min {} s", ms / 60_000, (ms % 60_000) / 1000)
    }
}

fn sev_class(s: Severity) -> &'static str {
    match s {
        Severity::Critical => "crit",
        Severity::High => "high",
        Severity::Medium => "med",
        Severity::Low => "low",
        Severity::Unknown => "unk",
    }
}

fn pill(s: Severity) -> String {
    format!("<span class=\"sev {}\">{}</span>", sev_class(s), s.label())
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn tool_name(report: &Report, id: &str) -> String {
    report
        .tools
        .iter()
        .find(|t| t.tool == id)
        .map_or_else(|| id.to_owned(), |t| t.name.clone())
}

/// Gitleaks or Trivy ran, so "0 secrets" means something.
fn secrets_scanned(r: &Report) -> bool {
    r.tools
        .iter()
        .any(|t| (t.tool == "gitleaks" || t.tool == "trivy") && t.status == ToolStatus::Ran)
}

fn verdict(r: &Report) -> (&'static str, String, String) {
    let c = &r.counts;
    let urgent = c.critical + c.high;
    let ran = r
        .tools
        .iter()
        .filter(|t| t.status == ToolStatus::Ran)
        .count();
    if ran == 0 {
        return (
            "warn",
            "No scanner ran".into(),
            "Install at least one scanner (see Coverage) and scan again. This report can't say anything about the project yet.".into(),
        );
    }
    if c.secrets > 0 {
        return (
            "bad",
            format!("{} found in the code", plural(c.secrets, "possible secret", "possible secrets")),
            "Treat every exposed credential as compromised: rotate it first, then remove it from the code.".into(),
        );
    }
    if urgent > 0 {
        let fixable = r
            .upgrades
            .iter()
            .filter(|u| u.to.is_some() && u.worst >= Severity::High)
            .count();
        return (
            "bad",
            format!(
                "{} need attention",
                plural(
                    urgent,
                    "critical or high-severity issue",
                    "critical or high-severity issues"
                )
            ),
            if fixable > 0 {
                format!(
                    "{} would fix the most serious ones. See Recommended upgrades.",
                    plural(fixable, "package upgrade", "package upgrades")
                )
            } else {
                "No fixed version is published for some of them yet; check each advisory for a workaround.".into()
            },
        );
    }
    if c.issues() > 0 {
        return (
            "warn",
            format!(
                "{}, none critical or high",
                plural(c.issues(), "known issue", "known issues")
            ),
            "Worth fixing in your next dependency update.".into(),
        );
    }
    if secrets_scanned(r) {
        (
            "ok",
            "No known vulnerabilities, secrets or risky settings found".into(),
            "Scanners only find what their databases know about; scan again after dependency changes.".into(),
        )
    } else {
        (
            "ok",
            "No known vulnerabilities found".into(),
            "Secrets were not checked: install Gitleaks or Trivy to include them.".into(),
        )
    }
}

/// Renders the report. `app_version` goes in the footer.
pub fn render(r: &Report, app_version: &str) -> String {
    let mut h = String::with_capacity(32 * 1024);
    let title = format!("Security report - {}", r.project_name);
    let _ = write!(
        h,
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<title>{}</title><style>{CSS}</style></head><body><main>",
        esc(&title)
    );

    // Header
    let _ = write!(
        h,
        "<header><div class=\"eyebrow\">Security report</div><h1>{}</h1>\
<div class=\"path mono\">{}</div><dl class=\"meta\">\
<div><dt>Scanned</dt><dd>{}</dd></div><div><dt>Took</dt><dd>{}</dd></div>\
<div><dt>Scanners</dt><dd>{} of {} ran</dd></div><div><dt>Secrets in history</dt><dd>{}</dd></div></dl></header>",
        esc(&r.project_name),
        esc(&r.path),
        utc(r.started_at),
        duration(r.finished_at.saturating_sub(r.started_at)),
        r.tools
            .iter()
            .filter(|t| t.status == ToolStatus::Ran)
            .count(),
        r.tools.len(),
        if r.options.history {
            "Scanned"
        } else {
            "Not scanned (current files only)"
        },
    );

    if r.cancelled {
        h.push_str("<div class=\"banner warn\"><strong>This scan was stopped early.</strong> Some scanners didn't run, so the results are incomplete.</div>");
    }

    let (tone, head, text) = verdict(r);
    let _ = write!(
        h,
        "<section class=\"verdict {tone}\"><h2>{}</h2><p>{}</p></section>",
        esc(&head),
        esc(&text)
    );

    summary(&mut h, r);
    changes(&mut h, r);
    upgrades(&mut h, r, &r.upgrades);
    vulnerabilities(&mut h, r);
    secrets(&mut h, r);
    in_file(
        &mut h,
        r,
        FindingKind::Misconfiguration,
        "Misconfigurations",
        "Risky settings in Dockerfiles, Compose, Kubernetes and Terraform files, from Trivy.",
    );
    in_file(
        &mut h,
        r,
        FindingKind::Code,
        "Code issues",
        "Risky patterns in the project's own code, from Semgrep. Some are false positives: check each one in context.",
    );
    coverage(&mut h, r);

    let _ = write!(
        h,
        "<footer><p>Generated by DevDock {} on {}. DevDock runs these scanners on your machine and merges their results; \
it does not scan by itself. Results are only as complete as each scanner's database: an empty report means no \
<em>known</em> issues, not no issues.</p></footer></main></body></html>",
        esc(app_version),
        utc(crate::time::now_ms())
    );
    h
}

fn summary(h: &mut String, r: &Report) {
    let c = &r.counts;
    let rows = [
        ("crit", "Critical", c.critical),
        ("high", "High", c.high),
        ("med", "Medium", c.medium),
        ("low", "Low", c.low),
        ("unk", "Unknown", c.unknown),
    ];
    h.push_str("<section><h2>Summary</h2><div class=\"cards\">");
    for (cls, label, n) in rows {
        if label == "Unknown" && n == 0 {
            continue;
        }
        let _ = write!(
            h,
            "<div class=\"card {cls}{}\"><span class=\"n\">{n}</span><span class=\"l\">{label}</span></div>",
            if n == 0 { " zero" } else { "" }
        );
    }
    if secrets_scanned(r) {
        let _ = write!(
            h,
            "<div class=\"card secret{}\"><span class=\"n\">{}</span><span class=\"l\">Secrets</span></div></div>",
            if c.secrets == 0 { " zero" } else { "" },
            c.secrets
        );
    } else {
        h.push_str("<div class=\"card zero\"><span class=\"n\">&mdash;</span><span class=\"l\">Secrets not scanned</span></div></div>");
    }
    let total = c.issues();
    if total > 0 {
        h.push_str("<div class=\"bar\" role=\"img\" aria-label=\"Issues by severity\">");
        for (cls, label, n) in rows {
            if n > 0 {
                let pct = n as f64 * 100.0 / total as f64;
                let _ = write!(
                    h,
                    "<span class=\"{cls}\" style=\"width:{pct:.2}%\" title=\"{label}: {n}\"></span>"
                );
            }
        }
        h.push_str("</div>");
    }
    let affected = r.upgrades.len();
    let fixable = r.upgrades.iter().filter(|u| u.to.is_some()).count();
    let mut extra = String::new();
    if c.misconfigs > 0 {
        let _ = write!(
            extra,
            " Plus {}.",
            plural(c.misconfigs, "misconfiguration", "misconfigurations")
        );
    }
    if c.code > 0 {
        let _ = write!(
            extra,
            " Plus {} in the code.",
            plural(c.code, "risky pattern", "risky patterns")
        );
    }
    let _ = write!(
        h,
        "<p class=\"muted\">{} in {}; {} with a fixed version available.{extra}</p></section>",
        plural(c.vulnerabilities(), "vulnerability", "vulnerabilities"),
        plural(affected, "package", "packages"),
        plural(fixable, "package", "packages"),
    );
}

fn changes(h: &mut String, r: &Report) {
    let Some(prev) = r.previous_at else { return };
    let new = r.findings.iter().filter(|f| f.is_new).count();
    let _ = write!(
        h,
        "<section><h2>Since the last scan</h2><p class=\"muted\">Compared with the scan of {}.</p>\
<div class=\"delta\"><div class=\"card {}\"><span class=\"n\">{new}</span><span class=\"l\">New</span></div>\
<div class=\"card {}\"><span class=\"n\">{}</span><span class=\"l\">Resolved</span></div></div>",
        utc(prev),
        if new > 0 { "high" } else { "zero" },
        if r.resolved.is_empty() {
            "zero"
        } else {
            "good"
        },
        r.resolved.len()
    );
    if !r.resolved.is_empty() {
        h.push_str("<table><thead><tr><th>Severity</th><th>Resolved</th><th>Package</th></tr></thead><tbody>");
        for x in &r.resolved {
            let pkg = match (&x.package, &x.version) {
                (Some(p), Some(v)) => format!("{p}@{v}"),
                (Some(p), None) => p.clone(),
                _ => String::new(),
            };
            let _ = write!(
                h,
                "<tr><td>{}</td><td><span class=\"mono\">{}</span> {}</td><td class=\"mono\">{}</td></tr>",
                pill(x.severity),
                esc(&x.id),
                esc(&x.title),
                esc(&pkg)
            );
        }
        h.push_str("</tbody></table>");
    }
    h.push_str("</section>");
}

fn upgrades(h: &mut String, _r: &Report, list: &[Upgrade]) {
    if list.is_empty() {
        return;
    }
    h.push_str(
        "<section><h2>Recommended upgrades</h2><p class=\"muted\">One row per installed package, worst first. \
Upgrading to the version shown fixes every listed advisory that has a fix. For a package you don't depend on directly, \
upgrade the dependency that brings it in, or pin it with your package manager's overrides.</p>\
<table><thead><tr><th>Worst</th><th>Package</th><th>Installed</th><th>Upgrade to</th><th>Fixes</th></tr></thead><tbody>",
    );
    for u in list {
        let to = u.to.as_deref().map_or_else(
            || "<span class=\"muted\">No fix yet</span>".to_owned(),
            |v| format!("<span class=\"mono to\">{}</span>", esc(v)),
        );
        let mut fixes = plural(u.fixes, "issue", "issues");
        if u.unfixed > 0 {
            let _ = write!(
                fixes,
                " <span class=\"muted\">({} without a fix)</span>",
                u.unfixed
            );
        }
        let _ = write!(
            h,
            "<tr><td>{}</td><td><span class=\"mono\">{}</span>{}{}</td><td class=\"mono\">{}</td><td>{to}</td><td>{fixes}</td></tr>",
            pill(u.worst),
            esc(&u.package),
            u.ecosystem
                .as_deref()
                .map(|e| format!(" <span class=\"tag\">{}</span>", esc(e)))
                .unwrap_or_default(),
            if u.dev {
                " <span class=\"tag\">dev only</span>"
            } else {
                ""
            },
            esc(u.version.as_deref().unwrap_or("?")),
        );
    }
    h.push_str("</tbody></table></section>");
}

fn finding_row(h: &mut String, r: &Report, f: &Finding) {
    let id = match f.url.as_deref().and_then(safe_url) {
        Some(url) => format!("<a href=\"{url}\" class=\"mono\">{}</a>", esc(&f.id)),
        None => format!("<span class=\"mono\">{}</span>", esc(&f.id)),
    };
    let aliases = if f.aliases.is_empty() {
        String::new()
    } else {
        format!(
            "<div class=\"aka mono\">{}</div>",
            esc(&f.aliases.join(", "))
        )
    };
    let tools: Vec<String> = f.tools.iter().map(|t| tool_name(r, t)).collect();
    let _ = write!(
        h,
        "<tr><td>{}{}</td><td class=\"adv\">{id}{aliases}</td><td><div>{}</div><div class=\"aka\">{}{}{}</div></td>\
<td class=\"mono\">{}@{}</td><td class=\"mono\">{}</td><td class=\"small\">{}</td></tr>",
        pill(f.severity),
        if f.is_new {
            " <span class=\"new\">New</span>"
        } else {
            ""
        },
        esc(if f.title.is_empty() {
            "(no description)"
        } else {
            &f.title
        }),
        f.location.as_deref().map(esc).unwrap_or_default(),
        f.cvss
            .map(|c| format!(" &middot; CVSS {c:.1}"))
            .unwrap_or_default(),
        if f.dev == Some(true) {
            " &middot; dev only"
        } else {
            ""
        },
        esc(f.package.as_deref().unwrap_or("?")),
        esc(f.version.as_deref().unwrap_or("?")),
        f.fixed_in.as_deref().map_or_else(|| "&mdash;".into(), esc),
        esc(&tools.join(", ")),
    );
}

fn vulnerabilities(h: &mut String, r: &Report) {
    let list: Vec<&Finding> = r
        .findings
        .iter()
        .filter(|f| f.kind == FindingKind::Vulnerability)
        .collect();
    if list.is_empty() {
        return;
    }
    let _ = write!(
        h,
        "<section><h2>Vulnerabilities <span class=\"count\">{}</span></h2>\
<table class=\"findings\"><thead><tr><th>Severity</th><th>Advisory</th><th>Issue</th><th>Package</th><th>Fixed in</th><th>Found by</th></tr></thead><tbody>",
        list.len()
    );
    for f in list {
        finding_row(h, r, f);
    }
    h.push_str("</tbody></table></section>");
}

fn secrets(h: &mut String, r: &Report) {
    let list: Vec<&Finding> = r
        .findings
        .iter()
        .filter(|f| f.kind == FindingKind::Secret)
        .collect();
    if list.is_empty() {
        return;
    }
    let _ = write!(
        h,
        "<section><h2>Possible secrets <span class=\"count\">{}</span></h2>\
<p class=\"muted\">The values themselves are not stored in this report. For each real credential: \
<strong>1.</strong> revoke or rotate it with its provider, <strong>2.</strong> remove it from the code and load it from \
the environment or a secret store, <strong>3.</strong> if it was ever committed, assume it's public: rewriting history \
doesn't recall copies already cloned. False positives can be listed in <span class=\"mono\">.gitleaksignore</span>.</p>\
<table><thead><tr><th>Rule</th><th>What</th><th>Where</th></tr></thead><tbody>",
        list.len()
    );
    for f in list {
        let place = format!(
            "{}{}{}",
            f.location.as_deref().unwrap_or("?"),
            f.line.map(|l| format!(":{l}")).unwrap_or_default(),
            f.commit
                .as_deref()
                .filter(|c| !c.is_empty())
                .map(|c| format!(" (commit {})", c.chars().take(8).collect::<String>()))
                .unwrap_or_default()
        );
        let _ = write!(
            h,
            "<tr><td class=\"mono\">{}{}</td><td>{}</td><td class=\"mono\">{}</td></tr>",
            esc(&f.id),
            if f.is_new {
                " <span class=\"new\">New</span>"
            } else {
                ""
            },
            esc(&f.title),
            esc(&place)
        );
    }
    h.push_str("</tbody></table></section>");
}

/// Misconfigurations and code issues: a rule at a file and line.
fn in_file(h: &mut String, r: &Report, kind: FindingKind, title: &str, intro: &str) {
    let list: Vec<&Finding> = r.findings.iter().filter(|f| f.kind == kind).collect();
    if list.is_empty() {
        return;
    }
    let _ = write!(
        h,
        "<section><h2>{title} <span class=\"count\">{}</span></h2><p class=\"muted\">{}</p><table class=\"findings\"><thead><tr><th>Severity</th><th>Rule</th><th>Issue</th><th>Where</th><th>Found by</th></tr></thead><tbody>",
        list.len(),
        esc(intro)
    );
    for f in list {
        let rule = match f.url.as_deref().and_then(safe_url) {
            Some(url) => format!("<a href=\"{url}\" class=\"mono\">{}</a>", esc(&f.id)),
            None => format!("<span class=\"mono\">{}</span>", esc(&f.id)),
        };
        let place = format!(
            "{}{}",
            f.location.as_deref().unwrap_or("?"),
            f.line.map(|l| format!(":{l}")).unwrap_or_default()
        );
        let tools: Vec<String> = f.tools.iter().map(|t| tool_name(r, t)).collect();
        let _ = write!(
            h,
            "<tr><td>{}{}</td><td class=\"adv\">{rule}</td><td><div>{}</div>{}</td><td class=\"mono\">{}</td><td class=\"small\">{}</td></tr>",
            pill(f.severity),
            if f.is_new {
                " <span class=\"new\">New</span>"
            } else {
                ""
            },
            esc(if f.title.is_empty() {
                "(no description)"
            } else {
                &f.title
            }),
            f.remediation
                .as_deref()
                .map(|x| format!("<div class=\"aka\">Fix: {}</div>", esc(x)))
                .unwrap_or_default(),
            esc(&place),
            esc(&tools.join(", ")),
        );
    }
    h.push_str("</tbody></table></section>");
}

fn coverage(h: &mut String, r: &Report) {
    h.push_str(
        "<section><h2>Coverage</h2><p class=\"muted\">What ran, and what didn't. A skipped scanner means that part of \
the project was not checked.</p><table><thead><tr><th>Scanner</th><th>Result</th><th>Findings</th><th>Time</th><th>Details</th></tr></thead><tbody>",
    );
    for t in &r.tools {
        let (cls, label) = match t.status {
            ToolStatus::Ran => ("ok", "Ran"),
            ToolStatus::Skipped => ("skip", "Skipped"),
            ToolStatus::Failed => ("fail", "Failed"),
            ToolStatus::Cancelled => ("skip", "Stopped"),
        };
        let mut details = String::new();
        if let Some(d) = &t.detail {
            let _ = write!(details, "<div>{}</div>", esc(d));
        }
        if let Some(i) = &t.install {
            let _ = write!(
                details,
                "<div class=\"aka\">Install: <span class=\"mono\">{}</span></div>",
                esc(i)
            );
        }
        if t.status != ToolStatus::Skipped
            && let Some(c) = &t.command
        {
            let _ = write!(details, "<div class=\"aka mono\">{}</div>", esc(c));
        }
        let _ = write!(
            h,
            "<tr><td>{}</td><td><span class=\"st {cls}\">{label}</span></td><td>{}</td><td>{}</td><td class=\"small\">{details}</td></tr>",
            esc(&t.name),
            if t.status == ToolStatus::Ran {
                t.findings.to_string()
            } else {
                "&mdash;".into()
            },
            if t.ms > 0 {
                duration(t.ms)
            } else {
                "&mdash;".into()
            },
        );
    }
    h.push_str("</tbody></table></section>");
}

const CSS: &str = r#"
:root{--bg:#fbfbfa;--panel:#fff;--text:#1d1d1f;--soft:#45454a;--muted:#6b6b72;--line:#e4e4e7;--mono:ui-monospace,"Cascadia Code",Consolas,monospace;
--crit:#9f1239;--crit-bg:#ffe4e6;--high:#c2410c;--high-bg:#ffedd5;--med:#a16207;--med-bg:#fef9c3;--low:#1d4ed8;--low-bg:#dbeafe;--unk:#52525b;--unk-bg:#f4f4f5;
--ok:#15803d;--ok-bg:#dcfce7;--bad-bg:#fff1f2;--bad-line:#fecdd3;--warn-bg:#fffbeb;--warn-line:#fde68a;--ok-line:#bbf7d0;--accent:#2f6fdf}
@media (prefers-color-scheme:dark){:root{--bg:#141416;--panel:#1c1c1f;--text:#ececef;--soft:#c4c4c9;--muted:#9a9aa2;--line:#2e2e33;
--crit:#fda4af;--crit-bg:#4c0519;--high:#fdba74;--high-bg:#431407;--med:#fde047;--med-bg:#3f3a06;--low:#93c5fd;--low-bg:#172554;--unk:#d4d4d8;--unk-bg:#27272a;
--ok:#86efac;--ok-bg:#052e16;--bad-bg:#2a0a12;--bad-line:#5f1424;--warn-bg:#2a2006;--warn-line:#5c4708;--ok-line:#14532d;--accent:#7aa7ff}}
*{box-sizing:border-box}html{-webkit-text-size-adjust:100%}
body{margin:0;background:var(--bg);color:var(--text);font:14px/1.55 "Segoe UI",system-ui,-apple-system,sans-serif}
main{max-width:1080px;margin:0 auto;padding:40px 24px 64px}
.mono{font-family:var(--mono);font-size:.92em}
header .eyebrow{font-size:11px;letter-spacing:.1em;text-transform:uppercase;color:var(--muted)}
h1{margin:4px 0 6px;font-size:30px;letter-spacing:-.02em;overflow-wrap:anywhere}
h2{margin:0 0 12px;font-size:18px;letter-spacing:-.01em}
.path{color:var(--muted);overflow-wrap:anywhere}
.meta{display:flex;flex-wrap:wrap;gap:8px 32px;margin:18px 0 0;padding:14px 0 0;border-top:1px solid var(--line)}
.meta dt{font-size:11px;letter-spacing:.06em;text-transform:uppercase;color:var(--muted)}.meta dd{margin:0;font-weight:600}
section{margin-top:28px;padding:22px 24px;background:var(--panel);border:1px solid var(--line);border-radius:12px}
.verdict h2{font-size:20px;margin-bottom:4px}.verdict p{margin:0;color:var(--soft)}
.verdict.bad{background:var(--bad-bg);border-color:var(--bad-line)}.verdict.bad h2{color:var(--crit)}
.verdict.warn{background:var(--warn-bg);border-color:var(--warn-line)}.verdict.warn h2{color:var(--med)}
.verdict.ok{background:var(--ok-bg);border-color:var(--ok-line)}.verdict.ok h2{color:var(--ok)}
.banner{margin-top:20px;padding:12px 16px;border-radius:10px;border:1px solid var(--warn-line);background:var(--warn-bg)}
.cards,.delta{display:grid;grid-template-columns:repeat(auto-fit,minmax(112px,1fr));gap:10px}
.delta{grid-template-columns:repeat(auto-fit,minmax(112px,160px))}
.card{display:flex;flex-direction:column;padding:12px 14px;border-radius:10px;background:var(--unk-bg)}
.card .n{font-size:26px;font-weight:700;line-height:1.1;font-variant-numeric:tabular-nums}.card .l{font-size:12px;color:var(--soft)}
.card.crit{background:var(--crit-bg);color:var(--crit)}.card.high{background:var(--high-bg);color:var(--high)}
.card.med{background:var(--med-bg);color:var(--med)}.card.low{background:var(--low-bg);color:var(--low)}
.card.secret{background:var(--crit-bg);color:var(--crit)}.card.good{background:var(--ok-bg);color:var(--ok)}
.card.zero{background:var(--unk-bg);color:var(--muted)}
.bar{display:flex;height:10px;margin:16px 0 8px;border-radius:99px;overflow:hidden;background:var(--unk-bg)}
.bar span{display:block;height:100%}.bar .crit{background:var(--crit)}.bar .high{background:var(--high)}.bar .med{background:var(--med)}.bar .low{background:var(--low)}.bar .unk{background:var(--unk)}
.muted{color:var(--muted)}p.muted{margin:0 0 14px;font-size:13px}
table{width:100%;border-collapse:collapse;font-size:13px}
th{text-align:left;font-size:11px;letter-spacing:.06em;text-transform:uppercase;color:var(--muted);font-weight:600;padding:8px 10px;border-bottom:1px solid var(--line)}
td{padding:9px 10px;border-bottom:1px solid var(--line);vertical-align:top;overflow-wrap:break-word}
td.adv .mono:first-child{white-space:nowrap}
tr:last-child td{border-bottom:0}
.sev{display:inline-block;padding:1px 8px;border-radius:99px;font-size:11.5px;font-weight:600;white-space:nowrap}
.sev.crit{background:var(--crit-bg);color:var(--crit)}.sev.high{background:var(--high-bg);color:var(--high)}
.sev.med{background:var(--med-bg);color:var(--med)}.sev.low{background:var(--low-bg);color:var(--low)}.sev.unk{background:var(--unk-bg);color:var(--unk)}
.new{display:inline-block;margin-top:3px;padding:0 6px;border-radius:4px;font-size:10.5px;font-weight:700;letter-spacing:.04em;text-transform:uppercase;background:var(--accent);color:var(--panel)}
.tag{display:inline-block;margin-left:6px;padding:0 6px;border:1px solid var(--line);border-radius:4px;font-size:11px;color:var(--muted)}
.to{color:var(--ok);font-weight:600}
.aka{margin-top:2px;font-size:12px;color:var(--muted)}.small{font-size:12.5px}
.count{margin-left:6px;font-size:13px;font-weight:500;color:var(--muted)}
.st{font-weight:600}.st.ok{color:var(--ok)}.st.fail{color:var(--crit)}.st.skip{color:var(--muted)}
a{color:var(--accent);text-decoration:none}a:hover{text-decoration:underline}
footer{margin-top:28px;font-size:12px;color:var(--muted)}
@media (max-width:640px){main{padding:24px 16px 48px}section{padding:16px}table.findings thead{display:none}
table.findings tr{display:block;padding:10px 0;border-bottom:1px solid var(--line)}table.findings td{display:block;border:0;padding:2px 0}}
@media print{body{background:#fff}section{break-inside:avoid-page;border-color:#ddd}main{padding:0}a{color:inherit}}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::merge;
    use crate::security::model::{Counts, ScanOptions, ToolRun};

    #[test]
    fn utc_dates() {
        assert_eq!(utc(0), "1970-01-01 00:00 UTC");
        assert_eq!(utc(1_763_474_700_000), "2025-11-18 14:05 UTC");
        assert_eq!(utc(951_782_400_000), "2000-02-29 00:00 UTC");
    }

    #[test]
    fn renders_and_escapes() {
        let mut f = Finding::vulnerability("grype", "GHSA-1", "<script>");
        f.severity = Severity::High;
        f.title = "x & y".into();
        f.url = Some("javascript:alert(1)".into());
        f.fixed_in = Some("2.0.0".into());
        let findings = merge::merge(vec![f]);
        let r = Report {
            version: 1,
            project_id: "p".into(),
            project_name: "shop \"<b>\"".into(),
            path: "D:\\dev\\shop".into(),
            started_at: 0,
            finished_at: 61_000,
            options: ScanOptions::default(),
            tools: vec![ToolRun {
                tool: "grype".into(),
                name: "Grype".into(),
                status: ToolStatus::Ran,
                detail: None,
                install: None,
                command: Some("grype dir:.".into()),
                findings: 1,
                ms: 61_000,
            }],
            counts: Counts::of(&findings),
            upgrades: merge::upgrades(&findings),
            findings,
            previous_at: None,
            resolved: Vec::new(),
            cancelled: false,
        };
        let html = render(&r, "1.3.0");
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
        assert!(!html.contains("javascript:"), "unsafe links are dropped");
        assert!(html.contains("x &amp; y"));
        assert!(html.contains("Recommended upgrades"));
        assert!(html.contains("need attention"));
        assert!(html.contains("1 min 1 s"));
    }
}
