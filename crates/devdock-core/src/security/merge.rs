//! Folding duplicate findings together, comparing against the previous scan, and
//! turning findings into an upgrade list ("bump X to Y, fixes N").
//!
//! Grype and the package manager's audit often report the same advisory. Two
//! findings are the same when they name the same package (and version, when both
//! know it) and share any id: `GHSA-…`, `CVE-…` or `RUSTSEC-…` aliases count.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::model::{Finding, FindingKind, Resolved, Severity};
use super::version;

fn same_vuln(a: &Finding, b: &Finding) -> bool {
    if a.kind != FindingKind::Vulnerability || b.kind != FindingKind::Vulnerability {
        return false;
    }
    let (Some(pa), Some(pb)) = (&a.package, &b.package) else {
        return false;
    };
    if !pa.eq_ignore_ascii_case(pb) {
        return false;
    }
    if let (Some(va), Some(vb)) = (&a.version, &b.version)
        && version::compare(va, vb) != std::cmp::Ordering::Equal
    {
        return false;
    }
    a.ids().any(|x| b.ids().any(|y| x.eq_ignore_ascii_case(y)))
}

fn absorb(into: &mut Finding, other: Finding) {
    for id in other.ids() {
        if !into.ids().any(|x| x.eq_ignore_ascii_case(id)) {
            into.aliases.push(id.to_owned());
        }
    }
    into.severity = into.severity.max(other.severity);
    into.cvss = match (into.cvss, other.cvss) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    };
    // Audit titles ("Prototype Pollution in lodash") read better than Grype's
    // descriptions, which are often a paragraph; keep the shorter one.
    if into.title.is_empty() || (!other.title.is_empty() && other.title.len() < into.title.len()) {
        into.title = other.title;
    }
    into.version = into.version.take().or(other.version);
    into.ecosystem = into.ecosystem.take().or(other.ecosystem);
    into.fixed_in = into.fixed_in.take().or(other.fixed_in);
    into.location = into.location.take().or(other.location);
    into.url = into.url.take().or(other.url);
    into.remediation = into.remediation.take().or(other.remediation);
    into.dev = match (into.dev, other.dev) {
        // Needed at runtime anywhere means it isn't dev-only.
        (Some(a), Some(b)) => Some(a && b),
        (a, b) => a.or(b),
    };
    for t in other.tools {
        if !into.tools.contains(&t) {
            into.tools.push(t);
        }
    }
}

/// The id used in the key and shown first: a GHSA id when there is one (npm and
/// Grype agree on those), else a CVE, else whatever the scanner gave.
fn canonical(f: &Finding) -> String {
    let ids: Vec<&str> = f.ids().collect();
    let pick = |prefix: &str| {
        let mut m: Vec<&&str> = ids.iter().filter(|i| i.starts_with(prefix)).collect();
        m.sort();
        m.first().map(|s| (**s).to_owned())
    };
    pick("GHSA-")
        .or_else(|| pick("CVE-"))
        .unwrap_or_else(|| f.id.clone())
}

/// Merges duplicates, sets each finding's key, and sorts most severe first.
pub fn merge(findings: Vec<Finding>) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    for f in findings {
        if let Some(existing) = out.iter_mut().find(|e| same_vuln(e, &f)) {
            absorb(existing, f);
        } else if let Some(existing) = out
            .iter_mut()
            .find(|e| f.kind.in_file() && e.kind == f.kind && e.key == f.key)
        {
            // The same line flagged by two scanners (Gitleaks and Trivy): one finding.
            existing.severity = existing.severity.max(f.severity);
            existing.remediation = existing.remediation.take().or(f.remediation);
            existing.url = existing.url.take().or(f.url);
            for t in f.tools {
                if !existing.tools.contains(&t) {
                    existing.tools.push(t);
                }
            }
        } else {
            out.push(f);
        }
    }
    for f in &mut out {
        if f.kind == FindingKind::Vulnerability {
            let id = canonical(f);
            if id != f.id {
                let old = std::mem::replace(&mut f.id, id);
                f.aliases.retain(|a| *a != f.id);
                f.aliases.push(old);
            }
            f.aliases.sort();
            f.aliases.dedup();
            f.key = format!(
                "vuln:{}@{}:{}",
                f.package.as_deref().unwrap_or("").to_lowercase(),
                f.version.as_deref().unwrap_or("*"),
                f.id
            );
        }
        f.tools.sort();
    }
    out.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| (b.kind == FindingKind::Secret).cmp(&(a.kind == FindingKind::Secret)))
            .then_with(|| b.cvss.unwrap_or(0.0).total_cmp(&a.cvss.unwrap_or(0.0)))
            .then_with(|| a.package.cmp(&b.package))
            .then_with(|| a.id.cmp(&b.id))
    });
    out
}

/// Marks what's new since `previous` and lists what's gone. Without a previous
/// scan nothing is "new": everything is, which says nothing.
pub fn compare(current: &mut [Finding], previous: Option<&[Finding]>) -> Vec<Resolved> {
    let Some(previous) = previous else {
        return Vec::new();
    };
    let before: HashSet<&str> = previous.iter().map(|f| f.key.as_str()).collect();
    let now: HashSet<String> = current.iter().map(|f| f.key.clone()).collect();
    for f in current.iter_mut() {
        f.is_new = !before.contains(f.key.as_str());
    }
    previous
        .iter()
        .filter(|f| !now.contains(&f.key))
        .map(|f| Resolved {
            id: f.id.clone(),
            kind: f.kind,
            severity: f.severity,
            title: f.title.clone(),
            package: f.package.clone(),
            version: f.version.clone(),
        })
        .collect()
}

/// One package to upgrade, and what upgrading it fixes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Upgrade {
    pub package: String,
    pub version: Option<String>,
    pub ecosystem: Option<String>,
    /// The version that fixes every advisory with a known fix; `None` when none has one.
    pub to: Option<String>,
    pub fixes: usize,
    /// Advisories with no fixed version yet.
    pub unfixed: usize,
    pub worst: Severity,
    pub ids: Vec<String>,
    pub dev: bool,
}

/// Groups vulnerabilities by installed package, worst first.
pub fn upgrades(findings: &[Finding]) -> Vec<Upgrade> {
    let mut by: HashMap<(String, Option<String>), Upgrade> = HashMap::new();
    let mut order: Vec<(String, Option<String>)> = Vec::new();
    for f in findings
        .iter()
        .filter(|f| f.kind == FindingKind::Vulnerability)
    {
        let Some(pkg) = &f.package else { continue };
        let k = (pkg.clone(), f.version.clone());
        let u = by.entry(k.clone()).or_insert_with(|| {
            order.push(k);
            Upgrade {
                package: pkg.clone(),
                version: f.version.clone(),
                ecosystem: f.ecosystem.clone(),
                to: None,
                fixes: 0,
                unfixed: 0,
                worst: Severity::Unknown,
                ids: Vec::new(),
                dev: true,
            }
        });
        u.worst = u.worst.max(f.severity);
        u.ids.push(f.id.clone());
        u.dev &= f.dev == Some(true);
        if f.fixed_in.is_some() {
            u.fixes += 1;
            u.to = version::max(u.to.take(), f.fixed_in.clone());
        } else {
            u.unfixed += 1;
        }
    }
    let mut list: Vec<Upgrade> = order.into_iter().filter_map(|k| by.remove(&k)).collect();
    list.sort_by(|a, b| {
        b.worst
            .cmp(&a.worst)
            .then_with(|| (b.fixes + b.unfixed).cmp(&(a.fixes + a.unfixed)))
            .then_with(|| a.package.cmp(&b.package))
    });
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vuln(tool: &str, id: &str, pkg: &str, ver: Option<&str>, sev: Severity) -> Finding {
        let mut f = Finding::vulnerability(tool, id, pkg);
        f.version = ver.map(str::to_owned);
        f.severity = sev;
        f
    }

    #[test]
    fn grype_and_audit_fold_into_one() {
        let mut a = vuln("grype", "GHSA-1", "cookie", Some("0.6.0"), Severity::Low);
        a.title = "A long description of the cookie problem from the database".into();
        a.aliases = vec!["CVE-9".into()];
        let mut b = vuln("pnpm", "GHSA-1", "cookie", Some("0.6.0"), Severity::Medium);
        b.title = "cookie bug".into();
        b.fixed_in = Some("0.7.0".into());
        let c = vuln("pnpm", "GHSA-1", "cookie", Some("0.5.0"), Severity::Low);
        let out = merge(vec![a, b, c]);
        assert_eq!(out.len(), 2, "different versions stay apart");
        let top = &out[0];
        assert_eq!(top.tools, vec!["grype", "pnpm"]);
        assert_eq!(top.severity, Severity::Medium);
        assert_eq!(top.title, "cookie bug");
        assert_eq!(top.fixed_in.as_deref(), Some("0.7.0"));
        assert_eq!(top.key, "vuln:cookie@0.6.0:GHSA-1");
    }

    #[test]
    fn aliases_link_rustsec_to_ghsa() {
        let mut a = vuln(
            "cargo-audit",
            "RUSTSEC-2024-1",
            "glib",
            Some("0.18.5"),
            Severity::Unknown,
        );
        a.aliases = vec!["GHSA-x".into()];
        let b = vuln("grype", "GHSA-x", "glib", Some("0.18.5"), Severity::Medium);
        let out = merge(vec![a, b]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "GHSA-x", "GHSA id wins");
        assert_eq!(out[0].aliases, vec!["RUSTSEC-2024-1"]);
        assert_eq!(out[0].severity, Severity::Medium);
    }

    #[test]
    fn unknown_version_matches_a_known_one() {
        let a = vuln("bun", "GHSA-2", "lodash", None, Severity::High);
        let b = vuln("grype", "GHSA-2", "lodash", Some("4.17.15"), Severity::High);
        let out = merge(vec![a, b]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].version.as_deref(), Some("4.17.15"));
    }

    #[test]
    fn new_and_resolved() {
        let prev = merge(vec![
            vuln("grype", "GHSA-old", "a", Some("1"), Severity::High),
            vuln("grype", "GHSA-kept", "b", Some("1"), Severity::Low),
        ]);
        let mut now = merge(vec![
            vuln("grype", "GHSA-kept", "b", Some("1"), Severity::Low),
            vuln("grype", "GHSA-new", "c", Some("1"), Severity::Low),
        ]);
        let gone = compare(&mut now, Some(&prev));
        assert_eq!(gone.len(), 1);
        assert_eq!(gone[0].id, "GHSA-old");
        let new: Vec<_> = now
            .iter()
            .filter(|f| f.is_new)
            .map(|f| f.id.as_str())
            .collect();
        assert_eq!(new, vec!["GHSA-new"]);
        let mut first = now.clone();
        assert!(compare(&mut first, None).is_empty());
    }

    #[test]
    fn upgrade_to_the_highest_fix() {
        let mut a = vuln("x", "GHSA-a", "kit", Some("2.70.4"), Severity::High);
        a.fixed_in = Some("3.1.1".into());
        let mut b = vuln("x", "GHSA-b", "kit", Some("2.70.4"), Severity::Low);
        b.fixed_in = Some("3.0.0".into());
        let c = vuln("x", "GHSA-c", "pico", Some("4.0.7"), Severity::High);
        let ups = upgrades(&merge(vec![a, b, c]));
        assert_eq!(ups[0].package, "kit");
        assert_eq!(ups[0].to.as_deref(), Some("3.1.1"));
        assert_eq!(ups[0].fixes, 2);
        assert_eq!(ups[1].unfixed, 1);
        assert_eq!(ups[1].to, None);
    }
    #[test]
    fn same_secret_from_two_scanners() {
        let mut a = Finding::at(
            "gitleaks",
            FindingKind::Secret,
            "aws-access-token",
            Some(".env".into()),
            Some(2),
        );
        a.severity = Severity::High;
        let mut b = Finding::at(
            "trivy",
            FindingKind::Secret,
            "aws-access-key-id",
            Some(".env".into()),
            Some(2),
        );
        b.severity = Severity::Critical;
        let out = merge(vec![a, b]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].tools, vec!["gitleaks", "trivy"]);
        assert_eq!(out[0].severity, Severity::Critical);
    }
}
