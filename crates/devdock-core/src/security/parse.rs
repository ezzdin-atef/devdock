//! Turning each scanner's JSON into [`Finding`]s. Every parser is pure (text in,
//! findings out) so it can be tested against saved output.
//!
//! Package-manager audits come in four shapes, told apart by their content:
//! - npm 7+ (`auditReportVersion: 2`): `vulnerabilities` keyed by package.
//! - npm 6 style `advisories` keyed by id: pnpm, yarn berry 2/3.
//! - NDJSON, one advisory per line: yarn classic (`auditAdvisory`) and yarn 4.
//! - bun: package name -> list of advisories.

use serde_json::Value;

use super::model::{Finding, FindingKind, Severity};
use super::version;

pub type ParseResult = Result<Vec<Finding>, String>;

fn s(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
}

fn invalid(tool: &str, err: &serde_json::Error) -> String {
    format!("{tool} printed something DevDock couldn't read as JSON ({err}).")
}

/// `GHSA-xxxx` from `https://github.com/advisories/GHSA-xxxx`.
fn ghsa_in(url: &str) -> Option<String> {
    url.rsplit('/')
        .next()
        .filter(|seg| seg.starts_with("GHSA-"))
        .map(str::to_owned)
}

/// The first fixed version from a "patched" range: `>=0.7.0` -> `0.7.0`,
/// `^1.2.3` -> `1.2.3`. `<0.0.0` (npm's "no patch") gives `None`.
pub fn fix_from_patched(range: &str) -> Option<String> {
    let first = range.split("||").next()?.trim();
    if first.starts_with('<') {
        return None;
    }
    let v = first
        .split([' ', ','])
        .find(|t| !t.is_empty())?
        .trim_start_matches(['>', '=', '^', '~']);
    Some(v.to_owned()).filter(|v| !v.is_empty() && v != "0.0.0")
}

/// The first fixed version from a "vulnerable" range: the upper bound of its last
/// part. `<4.17.21` -> `4.17.21`, `>=2.0.0 <2.0.3` -> `2.0.3`; `<=x` names no fix.
pub fn fix_from_vulnerable(range: &str) -> Option<String> {
    let last = range.rsplit("||").next()?.trim();
    last.split([' ', ','])
        .filter_map(|part| part.strip_prefix('<'))
        .rfind(|p| !p.starts_with('=') && !p.is_empty())
        .map(str::to_owned)
}

fn short_title(text: &str) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    if line.chars().count() > 160 {
        let cut: String = line.chars().take(157).collect();
        format!("{}...", cut.trim_end())
    } else {
        line.to_owned()
    }
}

// --- Grype --------------------------------------------------------------------

pub fn grype(text: &str) -> ParseResult {
    let doc: Value = serde_json::from_str(text).map_err(|e| invalid("Grype", &e))?;
    let Some(matches) = doc.get("matches").and_then(Value::as_array) else {
        return Err("Grype's output had no list of matches.".into());
    };
    let mut out = Vec::new();
    for m in matches {
        let (Some(vuln), Some(art)) = (m.get("vulnerability"), m.get("artifact")) else {
            continue;
        };
        let (Some(id), Some(name)) = (s(vuln, "id"), s(art, "name")) else {
            continue;
        };
        let mut f = Finding::vulnerability("grype", &id, &name);
        f.version = s(art, "version");
        f.ecosystem = s(art, "type");
        f.location = art
            .get("locations")
            .and_then(Value::as_array)
            .and_then(|l| l.first())
            .and_then(|l| s(l, "path"))
            .map(|p| p.trim_start_matches(['/', '\\']).to_owned());
        f.aliases = m
            .get("relatedVulnerabilities")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|r| s(r, "id"))
            .filter(|a| *a != id)
            .collect();
        f.title = short_title(&s(vuln, "description").unwrap_or_default());
        f.url = s(vuln, "dataSource").or_else(|| {
            vuln.get("urls")
                .and_then(Value::as_array)
                .and_then(|u| u.first())
                .and_then(Value::as_str)
                .map(str::to_owned)
        });
        f.cvss = vuln
            .get("cvss")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|c| c.pointer("/metrics/baseScore").and_then(Value::as_f64))
            .filter(|x| *x > 0.0)
            .reduce(f64::max);
        f.severity = Severity::parse(&s(vuln, "severity").unwrap_or_default());
        if f.severity == Severity::Unknown {
            f.severity = f.cvss.map_or(Severity::Unknown, Severity::from_score);
        }
        let fix = vuln.get("fix");
        if fix.and_then(|x| s(x, "state")).as_deref() == Some("fixed") {
            let versions: Vec<String> = fix
                .and_then(|x| x.get("versions"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect();
            f.fixed_in = version::first_fix(f.version.as_deref(), &versions);
        }
        out.push(f);
    }
    Ok(out)
}

// --- Package-manager audits -----------------------------------------------------

/// Reads `npm` / `pnpm` / `yarn` / `bun` audit output in any of their shapes.
/// `lock` is `package-lock.json`, used to learn installed versions for npm 7+.
pub fn node_audit(tool: &str, text: &str, lock: Option<&Value>) -> ParseResult {
    let text = text.trim_start_matches('\u{feff}').trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    if let Ok(doc) = serde_json::from_str::<Value>(text) {
        if let Some(err) = doc.get("error") {
            let msg = s(err, "summary")
                .or_else(|| s(err, "message"))
                .or_else(|| err.as_str().map(str::to_owned))
                .unwrap_or_else(|| "the audit failed".into());
            return Err(short_title(&msg));
        }
        if doc.get("auditReportVersion").is_some() {
            return Ok(npm_v2(tool, &doc, lock));
        }
        if let Some(adv) = doc.get("advisories").and_then(Value::as_object) {
            return Ok(adv.values().flat_map(|a| advisory(tool, a)).collect());
        }
        if let Some(map) = doc.as_object()
            && map.values().all(Value::is_array)
        {
            return Ok(bun(tool, map));
        }
        // A single NDJSON line parses as a whole document too.
        if let Some(f) = ndjson_line(tool, &doc) {
            return Ok(f);
        }
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut read_any = false;
    for line in text.lines().map(str::trim).filter(|l| l.starts_with('{')) {
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            read_any = true;
            if let Some(found) = ndjson_line(tool, &v) {
                out.extend(found);
            }
        }
    }
    if read_any {
        Ok(out)
    } else {
        Err(format!(
            "{tool} printed something DevDock couldn't read: {}",
            short_title(text)
        ))
    }
}

fn ndjson_line(tool: &str, v: &Value) -> Option<Vec<Finding>> {
    // yarn classic
    if v.get("type").and_then(Value::as_str) == Some("auditAdvisory") {
        return v.pointer("/data/advisory").map(|a| advisory(tool, a));
    }
    // yarn 4: { value: "pkg", children: { ID, Issue, URL, Severity, ... } }
    let (Some(name), Some(c)) = (s(v, "value"), v.get("children")) else {
        return None;
    };
    let url = s(c, "URL");
    let id = url.as_deref().and_then(ghsa_in).or_else(|| {
        c.get("ID")
            .map(|id| format!("NPM-{}", id.to_string().trim_matches('"')))
    })?;
    let versions: Vec<String> = c
        .get("Tree Versions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|x| x.as_str().map(str::to_owned))
        .collect();
    let base = {
        let mut f = Finding::vulnerability(tool, &id, &name);
        f.ecosystem = Some("npm".into());
        f.title = short_title(&s(c, "Issue").unwrap_or_default());
        f.severity = Severity::parse(&s(c, "Severity").unwrap_or_default());
        f.fixed_in = s(c, "Vulnerable Versions").and_then(|r| fix_from_vulnerable(&r));
        f.url = url;
        f.location = Some("yarn.lock".into());
        f
    };
    Some(per_version(base, versions))
}

/// One finding per installed version (`1.0.0` and `1.2.0` can both be affected).
fn per_version(base: Finding, versions: Vec<String>) -> Vec<Finding> {
    if versions.is_empty() {
        return vec![base];
    }
    versions
        .into_iter()
        .map(|v| Finding {
            version: Some(v),
            ..base.clone()
        })
        .collect()
}

/// An npm 6 style advisory (pnpm, yarn).
fn advisory(tool: &str, a: &Value) -> Vec<Finding> {
    let Some(name) = s(a, "module_name") else {
        return Vec::new();
    };
    let url = s(a, "url");
    let id = s(a, "github_advisory_id")
        .or_else(|| url.as_deref().and_then(ghsa_in))
        .or_else(|| a.get("id").map(|id| format!("NPM-{id}")))
        .unwrap_or_else(|| format!("NPM-{name}"));
    let mut base = Finding::vulnerability(tool, &id, &name);
    base.ecosystem = Some("npm".into());
    base.title = short_title(&s(a, "title").unwrap_or_default());
    base.severity = Severity::parse(&s(a, "severity").unwrap_or_default());
    base.fixed_in = s(a, "patched_versions")
        .and_then(|r| fix_from_patched(&r))
        .or_else(|| s(a, "vulnerable_versions").and_then(|r| fix_from_vulnerable(&r)));
    base.cvss = a
        .pointer("/cvss/score")
        .and_then(Value::as_f64)
        .filter(|x| *x > 0.0);
    base.aliases = a
        .get("cves")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| c.as_str().map(str::to_owned))
        .collect();
    base.url = url;
    base.location = Some(
        match tool {
            "pnpm" => "pnpm-lock.yaml",
            "yarn" => "yarn.lock",
            _ => "package-lock.json",
        }
        .into(),
    );
    let findings = a.get("findings").and_then(Value::as_array);
    let mut out = Vec::new();
    let mut seen = Vec::<String>::new();
    for item in findings.into_iter().flatten() {
        let Some(v) = s(item, "version") else {
            continue;
        };
        if seen.contains(&v) {
            continue;
        }
        seen.push(v.clone());
        let mut f = base.clone();
        f.version = Some(v);
        f.dev = item.get("dev").and_then(Value::as_bool);
        out.push(f);
    }
    if out.is_empty() {
        out.push(base);
    }
    out
}

/// npm 7+: `vulnerabilities.<pkg>.via` holds the advisories (objects) and the
/// packages it's vulnerable through (strings, skipped: they have their own entry).
fn npm_v2(tool: &str, doc: &Value, lock: Option<&Value>) -> Vec<Finding> {
    let mut out = Vec::new();
    let Some(vulns) = doc.get("vulnerabilities").and_then(Value::as_object) else {
        return out;
    };
    let lock_pkgs = lock.and_then(|l| l.get("packages"));
    for (name, entry) in vulns {
        // Installed versions of this package, from the lockfile's `node_modules/...` nodes.
        let mut installed: Vec<(String, Option<bool>)> = Vec::new();
        for node in entry
            .get("nodes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(node) = node.as_str() else { continue };
            let Some(info) = lock_pkgs.and_then(|p| p.get(node)) else {
                continue;
            };
            if let Some(v) = s(info, "version")
                && !installed.iter().any(|(x, _)| *x == v)
            {
                installed.push((v, info.get("dev").and_then(Value::as_bool)));
            }
        }
        for via in entry
            .get("via")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if !via.is_object() {
                continue;
            }
            let pkg = s(via, "name").unwrap_or_else(|| name.clone());
            let url = s(via, "url");
            let id = url
                .as_deref()
                .and_then(ghsa_in)
                .or_else(|| via.get("source").map(|x| format!("NPM-{x}")))
                .unwrap_or_else(|| format!("NPM-{pkg}"));
            let mut base = Finding::vulnerability(tool, &id, &pkg);
            base.ecosystem = Some("npm".into());
            base.title = short_title(&s(via, "title").unwrap_or_default());
            base.severity = Severity::parse(&s(via, "severity").unwrap_or_default());
            base.fixed_in = s(via, "range").and_then(|r| fix_from_vulnerable(&r));
            base.cvss = via
                .pointer("/cvss/score")
                .and_then(Value::as_f64)
                .filter(|x| *x > 0.0);
            base.url = url;
            base.location = Some("package-lock.json".into());
            if installed.is_empty() {
                out.push(base);
            } else {
                for (v, dev) in &installed {
                    let mut f = base.clone();
                    f.version = Some(v.clone());
                    f.dev = *dev;
                    out.push(f);
                }
            }
        }
    }
    out
}

fn bun(tool: &str, map: &serde_json::Map<String, Value>) -> Vec<Finding> {
    let mut out = Vec::new();
    for (name, list) in map {
        for a in list.as_array().into_iter().flatten() {
            let url = s(a, "url");
            let id = url
                .as_deref()
                .and_then(ghsa_in)
                .or_else(|| a.get("id").map(|x| format!("NPM-{x}")))
                .unwrap_or_else(|| format!("NPM-{name}"));
            let mut f = Finding::vulnerability(tool, &id, name);
            f.ecosystem = Some("npm".into());
            f.title = short_title(&s(a, "title").unwrap_or_default());
            f.severity = Severity::parse(&s(a, "severity").unwrap_or_default());
            f.fixed_in = s(a, "vulnerable_versions").and_then(|r| fix_from_vulnerable(&r));
            f.cvss = a
                .pointer("/cvss/score")
                .and_then(Value::as_f64)
                .filter(|x| *x > 0.0);
            f.url = url;
            f.location = Some("bun.lock".into());
            out.push(f);
        }
    }
    out
}

// --- cargo audit ----------------------------------------------------------------

pub fn cargo_audit(text: &str) -> ParseResult {
    let doc: Value = serde_json::from_str(text.trim()).map_err(|e| invalid("cargo audit", &e))?;
    let list = doc
        .pointer("/vulnerabilities/list")
        .and_then(Value::as_array)
        .ok_or("cargo audit's output had no vulnerability list.")?;
    let mut out = Vec::new();
    for item in list {
        let (Some(adv), Some(pkg)) = (item.get("advisory"), item.get("package")) else {
            continue;
        };
        let (Some(id), Some(name)) = (s(adv, "id"), s(pkg, "name")) else {
            continue;
        };
        let mut f = Finding::vulnerability("cargo-audit", &id, &name);
        f.version = s(pkg, "version");
        f.ecosystem = Some("rust-crate".into());
        f.title = short_title(&s(adv, "title").unwrap_or_default());
        f.aliases = adv
            .get("aliases")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|a| a.as_str().map(str::to_owned))
            .collect();
        f.url = s(adv, "url").or_else(|| Some(format!("https://rustsec.org/advisories/{id}")));
        let patched: Vec<String> = item
            .pointer("/versions/patched")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|p| p.as_str())
            .filter_map(fix_from_patched)
            .collect();
        f.fixed_in = version::first_fix(f.version.as_deref(), &patched);
        f.severity = Severity::parse(&s(adv, "severity").unwrap_or_default());
        f.location = Some("Cargo.lock".into());
        out.push(f);
    }
    Ok(out)
}

// --- Gitleaks -------------------------------------------------------------------

/// Gitleaks' JSON report. The secret itself (`Secret`, `Match`) is never kept: a
/// report that repeats the secret would be a leak of its own.
pub fn gitleaks(text: &str) -> ParseResult {
    let text = text.trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let doc: Value = serde_json::from_str(text).map_err(|e| invalid("Gitleaks", &e))?;
    let Some(items) = doc.as_array() else {
        return Err("Gitleaks' report was not a list.".into());
    };
    let mut out: Vec<Finding> = Vec::new();
    for item in items {
        let Some(rule) = s(item, "RuleID") else {
            continue;
        };
        let file = s(item, "File").map(|f| tidy_path(&f, ""));
        let mut f = Finding::at(
            "gitleaks",
            FindingKind::Secret,
            &rule,
            file,
            line_of(item, "StartLine"),
        );
        f.commit = s(item, "Commit");
        if let Some(c) = &f.commit {
            f.key = format!("{}:{c}", f.key);
        }
        f.severity = Severity::High;
        f.title = s(item, "Description").unwrap_or_else(|| format!("Possible secret ({rule})"));
        if !out.iter().any(|o| o.key == f.key) {
            out.push(f);
        }
    }
    Ok(out)
}

fn line_of(v: &Value, key: &str) -> Option<u32> {
    v.get(key)
        .and_then(Value::as_u64)
        .and_then(|l| u32::try_from(l).ok())
        .filter(|l| *l > 0)
}

/// Project-relative, forward slashes: `.\src\x.ts` or `C:\dev\shop\src\x.ts` -> `src/x.ts`.
pub fn tidy_path(path: &str, root: &str) -> String {
    let p = path.replace('\\', "/");
    let r = root.replace('\\', "/");
    let r = r.trim_end_matches('/');
    let rel = match p.get(..r.len()) {
        Some(head) if !r.is_empty() && p.len() > r.len() && head.eq_ignore_ascii_case(r) => {
            &p[r.len()..]
        }
        _ => &p,
    };
    rel.trim_start_matches("./")
        .trim_start_matches('/')
        .to_owned()
}

fn array<'a>(v: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    v.get(key).and_then(Value::as_array).into_iter().flatten()
}

// --- Trivy ----------------------------------------------------------------------

/// `trivy fs --format json`: vulnerabilities, misconfigurations and secrets per target.
pub fn trivy(text: &str) -> ParseResult {
    let doc: Value = serde_json::from_str(text.trim()).map_err(|e| invalid("Trivy", &e))?;
    let mut out: Vec<Finding> = Vec::new();
    for r in array(&doc, "Results") {
        let target = s(r, "Target").map(|t| tidy_path(&t, ""));
        for v in array(r, "Vulnerabilities") {
            let (Some(id), Some(name)) = (s(v, "VulnerabilityID"), s(v, "PkgName")) else {
                continue;
            };
            let mut f = Finding::vulnerability("trivy", &id, &name);
            f.version = s(v, "InstalledVersion");
            f.ecosystem = s(r, "Type");
            f.location.clone_from(&target);
            f.title = short_title(
                &s(v, "Title")
                    .or_else(|| s(v, "Description"))
                    .unwrap_or_default(),
            );
            f.severity = Severity::parse(&s(v, "Severity").unwrap_or_default());
            f.url = s(v, "PrimaryURL");
            f.cvss = v
                .get("CVSS")
                .and_then(Value::as_object)
                .into_iter()
                .flat_map(|m| m.values())
                .filter_map(|c| {
                    c.get("V3Score")
                        .or_else(|| c.get("V40Score"))
                        .and_then(Value::as_f64)
                })
                .filter(|x| *x > 0.0)
                .reduce(f64::max);
            let fixes: Vec<String> = s(v, "FixedVersion")
                .unwrap_or_default()
                .split(',')
                .map(|x| x.trim().to_owned())
                .filter(|x| !x.is_empty())
                .collect();
            f.fixed_in = version::first_fix(f.version.as_deref(), &fixes);
            out.push(f);
        }
        for m in array(r, "Misconfigurations") {
            if s(m, "Status").as_deref() == Some("PASS") {
                continue;
            }
            let Some(id) = s(m, "AVDID").or_else(|| s(m, "ID")) else {
                continue;
            };
            let line = m
                .pointer("/CauseMetadata/StartLine")
                .and_then(Value::as_u64)
                .and_then(|l| u32::try_from(l).ok())
                .filter(|l| *l > 0);
            let mut f = Finding::at(
                "trivy",
                FindingKind::Misconfiguration,
                &id,
                target.clone(),
                line,
            );
            f.title = short_title(
                &s(m, "Title")
                    .or_else(|| s(m, "Message"))
                    .unwrap_or_default(),
            );
            f.severity = Severity::parse(&s(m, "Severity").unwrap_or_default());
            f.url = s(m, "PrimaryURL");
            f.remediation = s(m, "Resolution");
            if !out.iter().any(|o| o.key == f.key) {
                out.push(f);
            }
        }
        for x in array(r, "Secrets") {
            let Some(rule) = s(x, "RuleID") else {
                continue;
            };
            // `Match` holds the line with the secret (masked by Trivy); it's not kept.
            let mut f = Finding::at(
                "trivy",
                FindingKind::Secret,
                &rule,
                target.clone(),
                line_of(x, "StartLine"),
            );
            f.severity = Severity::High;
            f.title = s(x, "Title").unwrap_or_else(|| format!("Possible secret ({rule})"));
            if !out.iter().any(|o| o.key == f.key) {
                out.push(f);
            }
        }
    }
    Ok(out)
}

// --- OSV-Scanner ----------------------------------------------------------------

fn osv_group_score(groups: &[&Value], id: &str) -> Option<f64> {
    groups
        .iter()
        .find(|g| array(g, "ids").any(|i| i.as_str() == Some(id)))
        .and_then(|g| s(g, "max_severity"))
        .and_then(|x| x.parse::<f64>().ok())
        .filter(|x| *x > 0.0)
}

fn osv_fixes(v: &Value, name: &str) -> Vec<String> {
    array(v, "affected")
        .filter(|a| {
            a.pointer("/package/name")
                .and_then(Value::as_str)
                .is_none_or(|n| n.eq_ignore_ascii_case(name))
        })
        .flat_map(|a| array(a, "ranges"))
        .flat_map(|r| array(r, "events"))
        .filter_map(|e| s(e, "fixed"))
        .collect()
}

/// `osv-scanner --format json`. `root` makes the lockfile paths project-relative.
pub fn osv(text: &str, root: &str) -> ParseResult {
    let text = text.trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let doc: Value = serde_json::from_str(text).map_err(|e| invalid("OSV-Scanner", &e))?;
    let mut out = Vec::new();
    for r in array(&doc, "results") {
        let location = r
            .pointer("/source/path")
            .and_then(Value::as_str)
            .map(|p| tidy_path(p, root));
        for p in array(r, "packages") {
            let Some(pkg) = p.get("package") else {
                continue;
            };
            let Some(name) = s(pkg, "name") else {
                continue;
            };
            let groups: Vec<&Value> = array(p, "groups").collect();
            for v in array(p, "vulnerabilities") {
                let Some(id) = s(v, "id") else { continue };
                let mut f = Finding::vulnerability("osv-scanner", &id, &name);
                f.version = s(pkg, "version");
                f.ecosystem = s(pkg, "ecosystem").map(|e| e.to_lowercase());
                f.location.clone_from(&location);
                f.aliases = array(v, "aliases")
                    .filter_map(|a| a.as_str().map(str::to_owned))
                    .collect();
                f.title = short_title(
                    &s(v, "summary")
                        .or_else(|| s(v, "details"))
                        .unwrap_or_default(),
                );
                f.cvss = osv_group_score(&groups, &id);
                f.severity = match f.cvss {
                    Some(score) => Severity::from_score(score),
                    None => Severity::parse(
                        v.pointer("/database_specific/severity")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                    ),
                };
                f.fixed_in = version::first_fix(f.version.as_deref(), &osv_fixes(v, &name));
                f.url = Some(if id.starts_with("GHSA-") {
                    format!("https://github.com/advisories/{id}")
                } else {
                    format!("https://osv.dev/vulnerability/{id}")
                });
                out.push(f);
            }
        }
    }
    Ok(out)
}

// --- Semgrep --------------------------------------------------------------------

/// `semgrep scan --json`: risky code patterns.
pub fn semgrep(text: &str, root: &str) -> ParseResult {
    let doc: Value = serde_json::from_str(text.trim()).map_err(|e| invalid("Semgrep", &e))?;
    let Some(results) = doc.get("results").and_then(Value::as_array) else {
        let first = array(&doc, "errors")
            .next()
            .and_then(|e| s(e, "message"))
            .unwrap_or_else(|| "Semgrep's output had no results list.".into());
        return Err(short_title(&first));
    };
    let mut out: Vec<Finding> = Vec::new();
    for r in results {
        let Some(rule) = s(r, "check_id") else {
            continue;
        };
        let path = s(r, "path").map(|p| tidy_path(&p, root));
        let line = r
            .pointer("/start/line")
            .and_then(Value::as_u64)
            .and_then(|l| u32::try_from(l).ok());
        // Rule ids are long dotted paths; the last part reads best.
        let short = rule.rsplit('.').next().unwrap_or(&rule).to_owned();
        let mut f = Finding::at("semgrep", FindingKind::Code, &short, path, line);
        f.aliases = vec![rule.clone()];
        let extra = r.get("extra");
        f.title = short_title(&extra.and_then(|e| s(e, "message")).unwrap_or_default());
        let sev = extra.and_then(|e| s(e, "severity")).unwrap_or_default();
        f.severity = match sev.to_ascii_uppercase().as_str() {
            "CRITICAL" => Severity::Critical,
            "ERROR" | "HIGH" => Severity::High,
            "WARNING" | "MEDIUM" => Severity::Medium,
            "INFO" | "LOW" => Severity::Low,
            _ => Severity::Unknown,
        };
        let meta = extra.and_then(|e| e.get("metadata"));
        f.url = meta.and_then(|m| s(m, "source")).or_else(|| {
            meta.and_then(|m| array(m, "references").next())
                .and_then(|u| u.as_str().map(str::to_owned))
        });
        f.remediation = extra
            .and_then(|e| s(e, "fix"))
            .map(|x| format!("Suggested change: {x}"));
        if let Some(cwe) = meta.and_then(|m| m.get("cwe")) {
            let first = cwe
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .or_else(|| cwe.as_str());
            if let Some(c) = first {
                f.aliases
                    .push(c.split(':').next().unwrap_or(c).trim().to_owned());
            }
        }
        if !out.iter().any(|o| o.key == f.key) {
            out.push(f);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_give_the_first_fix() {
        assert_eq!(fix_from_vulnerable("<4.17.21").as_deref(), Some("4.17.21"));
        assert_eq!(
            fix_from_vulnerable(">=2.0.0 <2.0.3").as_deref(),
            Some("2.0.3")
        );
        assert_eq!(fix_from_vulnerable("<=1.2.3"), None);
        assert_eq!(
            fix_from_vulnerable("<1.0.1 || >=2.0.0 <2.0.5").as_deref(),
            Some("2.0.5")
        );
        assert_eq!(fix_from_patched(">=0.7.0").as_deref(), Some("0.7.0"));
        assert_eq!(fix_from_patched(">=1.2.3, <2").as_deref(), Some("1.2.3"));
        assert_eq!(fix_from_patched("^0.19.5").as_deref(), Some("0.19.5"));
        assert_eq!(fix_from_patched("<0.0.0"), None);
        assert_eq!(fix_from_patched(">=0.0.0"), None);
    }

    const GRYPE: &str = r#"{"matches":[
      {"vulnerability":{"id":"GHSA-pxg6-pf52-xh8x","dataSource":"https://github.com/advisories/GHSA-pxg6-pf52-xh8x",
        "severity":"Low","description":"cookie accepts cookie name, path, and domain with out of bounds characters",
        "cvss":[{"metrics":{"baseScore":3.7}}],"fix":{"versions":["0.7.0"],"state":"fixed"}},
       "relatedVulnerabilities":[{"id":"CVE-2024-47764"}],
       "artifact":{"name":"cookie","version":"0.6.0","type":"npm","locations":[{"path":"/pnpm-lock.yaml"}]}},
      {"vulnerability":{"id":"GHSA-fc5q-39xc-vjpf","severity":"Unknown","description":"ReDoS",
        "cvss":[{"metrics":{"baseScore":7.5}},{"metrics":{"baseScore":8.7}}],"fix":{"versions":[],"state":"not-fixed"}},
       "artifact":{"name":"picomatch","version":"4.0.7","type":"npm","locations":[{"path":"/pnpm-lock.yaml"}]}}
    ],"source":{}}"#;

    #[test]
    fn grype_matches() {
        let f = grype(GRYPE).unwrap();
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].id, "GHSA-pxg6-pf52-xh8x");
        assert_eq!(f[0].aliases, vec!["CVE-2024-47764"]);
        assert_eq!(f[0].severity, Severity::Low);
        assert_eq!(f[0].fixed_in.as_deref(), Some("0.7.0"));
        assert_eq!(f[0].location.as_deref(), Some("pnpm-lock.yaml"));
        assert_eq!(f[0].version.as_deref(), Some("0.6.0"));
        // No severity word: the highest CVSS score decides.
        assert_eq!(f[1].severity, Severity::High);
        assert_eq!(f[1].cvss, Some(8.7));
        assert_eq!(f[1].fixed_in, None);
        assert!(grype("not json").is_err());
    }

    const PNPM: &str = r#"{"advisories":{"1103907":{"findings":[
        {"version":"0.6.0","paths":[".>a>cookie"],"dev":true},
        {"version":"0.6.0","paths":[".>b>cookie"],"dev":true},
        {"version":"0.5.0","paths":[".>c>cookie"],"dev":false}],
      "id":1103907,"title":"cookie accepts cookie name","module_name":"cookie",
      "vulnerable_versions":"<0.7.0","patched_versions":">=0.7.0","severity":"low",
      "github_advisory_id":"GHSA-pxg6-pf52-xh8x","cves":["CVE-2024-47764"],
      "url":"https://github.com/advisories/GHSA-pxg6-pf52-xh8x"}},"metadata":{}}"#;

    #[test]
    fn pnpm_advisories_one_per_version() {
        let f = node_audit("pnpm", PNPM, None).unwrap();
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].version.as_deref(), Some("0.6.0"));
        assert_eq!(f[0].dev, Some(true));
        assert_eq!(f[1].version.as_deref(), Some("0.5.0"));
        assert_eq!(f[0].fixed_in.as_deref(), Some("0.7.0"));
        assert_eq!(f[0].aliases, vec!["CVE-2024-47764"]);
        assert_eq!(f[0].location.as_deref(), Some("pnpm-lock.yaml"));
    }

    #[test]
    fn npm_v2_reads_versions_from_the_lockfile() {
        let report = r#"{"auditReportVersion":2,"vulnerabilities":{
          "lodash":{"name":"lodash","severity":"high","via":[
            {"source":1094499,"name":"lodash","title":"Prototype Pollution","url":"https://github.com/advisories/GHSA-p6mc-m468-83gw","severity":"high","cvss":{"score":7.4},"range":"<4.17.19"}],
            "nodes":["node_modules/lodash"],"fixAvailable":true},
          "parent":{"name":"parent","severity":"high","via":["lodash"],"nodes":["node_modules/parent"]}
        },"metadata":{}}"#;
        let lock: Value = serde_json::from_str(
            r#"{"packages":{"node_modules/lodash":{"version":"4.17.15","dev":true}}}"#,
        )
        .unwrap();
        let f = node_audit("npm", report, Some(&lock)).unwrap();
        assert_eq!(f.len(), 1, "string `via` entries are not findings");
        assert_eq!(f[0].id, "GHSA-p6mc-m468-83gw");
        assert_eq!(f[0].version.as_deref(), Some("4.17.15"));
        assert_eq!(f[0].fixed_in.as_deref(), Some("4.17.19"));
        assert_eq!(f[0].dev, Some(true));
        assert_eq!(f[0].cvss, Some(7.4));
    }

    #[test]
    fn npm_error_is_reported() {
        let err = node_audit("npm", r#"{"error":{"code":"ENOLOCK","summary":"This command requires an existing lockfile."}}"#, None).unwrap_err();
        assert!(err.contains("lockfile"), "{err}");
    }

    #[test]
    fn yarn_classic_and_berry_ndjson() {
        let classic = concat!(
            r#"{"type":"auditAdvisory","data":{"resolution":{"id":1,"path":"a>minimist"},"advisory":{"id":1179,"module_name":"minimist","title":"Prototype Pollution","severity":"moderate","patched_versions":">=1.2.6","url":"https://github.com/advisories/GHSA-xvch-5gv4-984h","findings":[{"version":"1.2.5"}]}}}"#,
            "\n",
            r#"{"type":"auditSummary","data":{}}"#
        );
        let f = node_audit("yarn", classic, None).unwrap();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].id, "GHSA-xvch-5gv4-984h");
        assert_eq!(f[0].severity, Severity::Medium);

        let berry = r#"{"value":"minimist","children":{"ID":1096,"Issue":"Prototype Pollution","URL":"https://github.com/advisories/GHSA-xvch-5gv4-984h","Severity":"critical","Vulnerable Versions":"<1.2.6","Tree Versions":["1.2.5","1.2.0"],"Dependents":["a@npm:1"]}}"#;
        let f = node_audit("yarn", berry, None).unwrap();
        assert_eq!(f.len(), 2);
        assert_eq!(f[1].version.as_deref(), Some("1.2.0"));
        assert_eq!(f[0].fixed_in.as_deref(), Some("1.2.6"));
        assert_eq!(f[0].severity, Severity::Critical);
    }

    #[test]
    fn bun_map_of_advisories() {
        let text = r#"{"lodash":[{"id":1,"url":"https://github.com/advisories/GHSA-35jh-r3h4-6jhm","title":"Command Injection","severity":"high","vulnerable_versions":"<4.17.21"}]}"#;
        let f = node_audit("bun", text, None).unwrap();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].fixed_in.as_deref(), Some("4.17.21"));
        assert!(node_audit("bun", "{}", None).unwrap().is_empty());
    }

    #[test]
    fn cargo_audit_list() {
        let text = r#"{"vulnerabilities":{"found":true,"count":1,"list":[{"advisory":{"id":"RUSTSEC-2024-0429","package":"glib","title":"Unsoundness in Iterator impls","aliases":["GHSA-wrw7-89jp-8q8g"],"url":"https://github.com/gtk-rs/gtk-rs-core/pull/1343"},"versions":{"patched":[">=0.20.0"],"unaffected":["<0.15.0"]},"package":{"name":"glib","version":"0.18.5"}}]}}"#;
        let f = cargo_audit(text).unwrap();
        assert_eq!(f[0].id, "RUSTSEC-2024-0429");
        assert_eq!(f[0].aliases, vec!["GHSA-wrw7-89jp-8q8g"]);
        assert_eq!(f[0].fixed_in.as_deref(), Some("0.20.0"));
        assert_eq!(f[0].version.as_deref(), Some("0.18.5"));
    }

    #[test]
    fn gitleaks_never_keeps_the_secret() {
        let text = r#"[{"RuleID":"aws-access-token","Description":"AWS access key","StartLine":3,"File":".\\config\\.env","Secret":"AKIAREALKEY","Match":"key=AKIAREALKEY","Commit":""},
                       {"RuleID":"aws-access-token","Description":"AWS access key","StartLine":3,"File":".\\config\\.env","Secret":"AKIAREALKEY","Match":"x","Commit":""}]"#;
        let f = gitleaks(text).unwrap();
        assert_eq!(f.len(), 1, "duplicates are dropped");
        assert_eq!(f[0].location.as_deref(), Some("config/.env"));
        assert_eq!(f[0].line, Some(3));
        assert_eq!(f[0].kind, FindingKind::Secret);
        let json = serde_json::to_string(&f).unwrap();
        assert!(!json.contains("AKIAREALKEY"), "{json}");
        assert!(gitleaks("").unwrap().is_empty());
    }
    #[test]
    fn paths_become_project_relative() {
        assert_eq!(tidy_path(".\\src\\x.ts", ""), "src/x.ts");
        assert_eq!(
            tidy_path("C:\\Dev\\Shop\\package-lock.json", "c:\\dev\\shop"),
            "package-lock.json"
        );
        assert_eq!(tidy_path("/pnpm-lock.yaml", ""), "pnpm-lock.yaml");
    }

    #[test]
    fn trivy_vulns_misconfigs_and_secrets() {
        let text = r#"{"SchemaVersion":2,"Results":[
          {"Target":"package-lock.json","Class":"lang-pkgs","Type":"npm","Vulnerabilities":[
            {"VulnerabilityID":"CVE-2024-47764","PkgName":"cookie","InstalledVersion":"0.6.0","FixedVersion":"0.7.0, 1.0.1","Severity":"LOW","Title":"cookie: out of bounds characters","PrimaryURL":"https://avd.aquasec.com/nvd/cve-2024-47764","CVSS":{"nvd":{"V3Score":3.7},"ghsa":{"V3Score":5.3}}}]},
          {"Target":"Dockerfile","Class":"config","Type":"dockerfile","Misconfigurations":[
            {"ID":"DS002","AVDID":"AVD-DS-0002","Title":"Image user should not be 'root'","Severity":"HIGH","Resolution":"Add 'USER <non root user name>' line","Status":"FAIL","PrimaryURL":"https://avd.aquasec.com/misconfig/ds002","CauseMetadata":{"StartLine":0}},
            {"ID":"DS001","AVDID":"AVD-DS-0001","Title":"ok","Severity":"LOW","Status":"PASS"}]},
          {"Target":"src/.env","Class":"secret","Secrets":[{"RuleID":"aws-access-key-id","Category":"AWS","Severity":"CRITICAL","Title":"AWS Access Key ID","StartLine":2,"Match":"KEY=****"}]}
        ]}"#;
        let f = trivy(text).unwrap();
        assert_eq!(f.len(), 3, "PASS checks are not findings");
        assert_eq!(f[0].fixed_in.as_deref(), Some("0.7.0"));
        assert_eq!(f[0].cvss, Some(5.3));
        assert_eq!(f[1].kind, FindingKind::Misconfiguration);
        assert_eq!(f[1].id, "AVD-DS-0002");
        assert_eq!(f[1].line, None);
        assert!(f[1].remediation.as_deref().unwrap().contains("USER"));
        assert_eq!(f[2].kind, FindingKind::Secret);
        assert_eq!(f[2].key, "secret:src/.env:2");
        assert!(!serde_json::to_string(&f).unwrap().contains("****"));
    }

    #[test]
    fn osv_results_with_groups() {
        let text = r#"{"results":[{"source":{"path":"C:\\dev\\shop\\package-lock.json","type":"lockfile"},"packages":[
          {"package":{"name":"lodash","version":"4.17.15","ecosystem":"npm"},
           "vulnerabilities":[{"id":"GHSA-35jh-r3h4-6jhm","aliases":["CVE-2021-23337"],"summary":"Command Injection in lodash",
             "affected":[{"package":{"name":"lodash","ecosystem":"npm"},"ranges":[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"4.17.21"}]}]}],
             "database_specific":{"severity":"HIGH"}}],
           "groups":[{"ids":["GHSA-35jh-r3h4-6jhm","CVE-2021-23337"],"max_severity":"7.2"}]}]}]}"#;
        let f = osv(text, "C:\\dev\\shop").unwrap();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].location.as_deref(), Some("package-lock.json"));
        assert_eq!(f[0].fixed_in.as_deref(), Some("4.17.21"));
        assert_eq!(f[0].severity, Severity::High);
        assert_eq!(f[0].cvss, Some(7.2));
        assert_eq!(f[0].aliases, vec!["CVE-2021-23337"]);
        assert!(osv("", "x").unwrap().is_empty());
    }

    #[test]
    fn semgrep_results() {
        let text = r#"{"results":[{"check_id":"javascript.lang.security.audit.code-string-concat.code-string-concat","path":"C:\\dev\\shop\\src\\eval.js",
          "start":{"line":12,"col":3},"end":{"line":12,"col":20},
          "extra":{"message":"Found data from an Express request flowing into eval.","severity":"ERROR",
            "metadata":{"cwe":["CWE-95: Improper Neutralization of Directives"],"source":"https://semgrep.dev/r/x"}}}],"errors":[]}"#;
        let f = semgrep(text, "C:\\dev\\shop").unwrap();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, FindingKind::Code);
        assert_eq!(f[0].id, "code-string-concat");
        assert_eq!(f[0].location.as_deref(), Some("src/eval.js"));
        assert_eq!(f[0].line, Some(12));
        assert_eq!(f[0].severity, Severity::High);
        assert!(f[0].aliases.contains(&"CWE-95".to_owned()));
        assert!(semgrep(r#"{"errors":[{"message":"Invalid config"}]}"#, "").is_err());
    }
}
