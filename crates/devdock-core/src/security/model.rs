//! What a security scan produces. One shape for every scanner, so findings from
//! Grype, a package manager's audit and Gitleaks can be merged and compared.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Unknown,
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    /// Reads the words scanners use: `moderate` (npm), `Negligible` (Grype), etc.
    pub fn parse(text: &str) -> Self {
        match text.trim().to_ascii_lowercase().as_str() {
            "critical" => Severity::Critical,
            "high" => Severity::High,
            "medium" | "moderate" => Severity::Medium,
            "low" | "negligible" | "info" | "informational" => Severity::Low,
            _ => Severity::Unknown,
        }
    }

    /// From a CVSS base score, for scanners that only give a score.
    pub fn from_score(score: f64) -> Self {
        if score >= 9.0 {
            Severity::Critical
        } else if score >= 7.0 {
            Severity::High
        } else if score >= 4.0 {
            Severity::Medium
        } else if score > 0.0 {
            Severity::Low
        } else {
            Severity::Unknown
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Severity::Critical => "Critical",
            Severity::High => "High",
            Severity::Medium => "Medium",
            Severity::Low => "Low",
            Severity::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FindingKind {
    /// A known vulnerability in a dependency.
    Vulnerability,
    /// A credential or key committed to the code.
    Secret,
    /// A risky setting in a Dockerfile, Compose, Kubernetes or Terraform file (Trivy).
    Misconfiguration,
    /// A risky pattern in the source code (Semgrep).
    Code,
}

impl FindingKind {
    /// Issues located at a file and line rather than in a package.
    pub fn in_file(self) -> bool {
        self != FindingKind::Vulnerability
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// Stable across scans, so the next scan can tell what's new and what's fixed.
    pub key: String,
    pub kind: FindingKind,
    /// The advisory id (`GHSA-…`, `CVE-…`, `RUSTSEC-…`) or the Gitleaks rule.
    pub id: String,
    /// Other ids for the same advisory.
    #[serde(default)]
    pub aliases: Vec<String>,
    pub severity: Severity,
    pub title: String,
    #[serde(default)]
    pub package: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    /// `npm`, `rust-crate`, `python`…
    #[serde(default)]
    pub ecosystem: Option<String>,
    /// The first version without the problem, when one exists.
    #[serde(default)]
    pub fixed_in: Option<String>,
    /// The lockfile that pulls the package in, or the file holding the secret.
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub line: Option<u32>,
    /// For secrets found in history.
    #[serde(default)]
    pub commit: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub cvss: Option<f64>,
    /// Only needed during development (`devDependencies`), when the audit says so.
    #[serde(default)]
    pub dev: Option<bool>,
    /// How to fix it, when the scanner says (misconfigurations, code issues).
    #[serde(default)]
    pub remediation: Option<String>,
    /// Which scanners reported it (ids from [`super::scanners`]).
    pub tools: Vec<String>,
    /// Not seen in the previous scan.
    #[serde(default)]
    pub is_new: bool,
}

impl Finding {
    pub fn vulnerability(tool: &str, id: &str, package: &str) -> Self {
        Finding {
            key: String::new(),
            kind: FindingKind::Vulnerability,
            id: id.to_owned(),
            aliases: Vec::new(),
            severity: Severity::Unknown,
            title: String::new(),
            package: Some(package.to_owned()),
            version: None,
            ecosystem: None,
            fixed_in: None,
            location: None,
            line: None,
            commit: None,
            url: None,
            cvss: None,
            dev: None,
            remediation: None,
            tools: vec![tool.to_owned()],
            is_new: false,
        }
    }

    /// A finding at a place in a file: a secret, misconfiguration or code issue.
    /// Keyed by place, not by rule, so two scanners flagging the same line agree.
    pub fn at(
        tool: &str,
        kind: FindingKind,
        id: &str,
        location: Option<String>,
        line: Option<u32>,
    ) -> Self {
        let prefix = match kind {
            FindingKind::Secret => "secret",
            FindingKind::Misconfiguration => "config",
            FindingKind::Code => "code",
            FindingKind::Vulnerability => "vuln",
        };
        let key = match kind {
            FindingKind::Secret => format!(
                "{prefix}:{}:{}",
                location.as_deref().unwrap_or(""),
                line.unwrap_or(0)
            ),
            _ => format!(
                "{prefix}:{id}:{}:{}",
                location.as_deref().unwrap_or(""),
                line.unwrap_or(0)
            ),
        };
        Finding {
            key,
            kind,
            id: id.to_owned(),
            aliases: Vec::new(),
            severity: Severity::Unknown,
            title: String::new(),
            package: None,
            version: None,
            ecosystem: None,
            fixed_in: None,
            location,
            line,
            commit: None,
            url: None,
            cvss: None,
            dev: None,
            remediation: None,
            tools: vec![tool.to_owned()],
            is_new: false,
        }
    }

    /// Every id this finding is known by.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.id.as_str()).chain(self.aliases.iter().map(String::as_str))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolStatus {
    Ran,
    /// Not installed, or nothing for it to scan in this project.
    Skipped,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRun {
    pub tool: String,
    pub name: String,
    pub status: ToolStatus,
    /// Why it was skipped or failed, in a sentence.
    #[serde(default)]
    pub detail: Option<String>,
    /// How to install it, when that's why it was skipped.
    #[serde(default)]
    pub install: Option<String>,
    /// The command line that ran.
    #[serde(default)]
    pub command: Option<String>,
    pub findings: usize,
    pub ms: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    /// By severity: every finding except secrets (vulnerabilities, misconfigurations
    /// and code issues).
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
    pub unknown: usize,
    /// Secrets are counted here and not by severity.
    pub secrets: usize,
    /// Of the severity counts, how many are misconfigurations and code issues.
    #[serde(default)]
    pub misconfigs: usize,
    #[serde(default)]
    pub code: usize,
}

impl Counts {
    pub fn of(findings: &[Finding]) -> Self {
        let mut c = Counts::default();
        for f in findings {
            match f.kind {
                FindingKind::Secret => {
                    c.secrets += 1;
                    continue;
                }
                FindingKind::Misconfiguration => c.misconfigs += 1,
                FindingKind::Code => c.code += 1,
                FindingKind::Vulnerability => {}
            }
            match f.severity {
                Severity::Critical => c.critical += 1,
                Severity::High => c.high += 1,
                Severity::Medium => c.medium += 1,
                Severity::Low => c.low += 1,
                Severity::Unknown => c.unknown += 1,
            }
        }
        c
    }

    /// Everything counted by severity.
    pub fn issues(&self) -> usize {
        self.critical + self.high + self.medium + self.low + self.unknown
    }

    pub fn vulnerabilities(&self) -> usize {
        self.issues() - self.misconfigs - self.code
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanOptions {
    /// Gitleaks also reads every commit, not just the files as they are now.
    #[serde(default)]
    pub history: bool,
}

/// A finding the previous scan had and this one doesn't.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolved {
    pub id: String,
    pub kind: FindingKind,
    pub severity: Severity,
    pub title: String,
    #[serde(default)]
    pub package: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub version: u32,
    pub project_id: String,
    pub project_name: String,
    pub path: String,
    pub started_at: u64,
    pub finished_at: u64,
    pub options: ScanOptions,
    pub tools: Vec<ToolRun>,
    /// Most severe first.
    pub findings: Vec<Finding>,
    pub counts: Counts,
    /// Vulnerabilities grouped by package, with the version that fixes them.
    #[serde(default)]
    pub upgrades: Vec<super::merge::Upgrade>,
    /// When the previous scan was taken; `None` for the first scan.
    #[serde(default)]
    pub previous_at: Option<u64>,
    /// Gone since the previous scan.
    #[serde(default)]
    pub resolved: Vec<Resolved>,
    /// The scan was stopped before every scanner ran.
    #[serde(default)]
    pub cancelled: bool,
}

/// What the project list needs: the counts and when.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub finished_at: u64,
    pub counts: Counts,
    pub new: usize,
}

impl Report {
    pub fn summary(&self) -> Summary {
        Summary {
            finished_at: self.finished_at,
            counts: self.counts,
            new: self.findings.iter().filter(|f| f.is_new).count(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_words_and_scores() {
        assert_eq!(Severity::parse("moderate"), Severity::Medium);
        assert_eq!(Severity::parse("Negligible"), Severity::Low);
        assert_eq!(Severity::parse("CRITICAL"), Severity::Critical);
        assert_eq!(Severity::parse("whatever"), Severity::Unknown);
        assert_eq!(Severity::from_score(9.8), Severity::Critical);
        assert_eq!(Severity::from_score(7.5), Severity::High);
        assert_eq!(Severity::from_score(5.3), Severity::Medium);
        assert_eq!(Severity::from_score(0.0), Severity::Unknown);
        assert!(Severity::Critical > Severity::High && Severity::Low > Severity::Unknown);
    }
}
