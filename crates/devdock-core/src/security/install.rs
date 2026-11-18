//! The Security tab's Install button.
//!
//! - Release binaries (Grype, Trivy, OSV-Scanner, Gitleaks): the latest GitHub
//!   release's Windows build, downloaded with Windows' own `curl.exe`, checked
//!   against the SHA-256 the project publishes, unpacked with Windows' own
//!   `tar.exe`, and put in DevDock's tools folder. Nothing system-wide changes and
//!   no admin rights are needed.
//! - Semgrep: `pip install` into a Python environment of its own in the tools folder.
//! - cargo audit: `cargo install`, into cargo's own bin folder.
//!
//! A download whose checksum doesn't match, or that has none, is thrown away.

use std::cell::Cell;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use serde_json::Value;

use super::scanners::{
    self, ExecError, Locator, Method, ScannerDef, Watch, cargo_bin, exec_watched, last_line, quote,
};
use crate::run::ProcessTree;

const API_TIMEOUT: Duration = Duration::from_secs(60);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(20 * 60);
const PIP_TIMEOUT: Duration = Duration::from_secs(20 * 60);
const CARGO_TIMEOUT: Duration = Duration::from_secs(40 * 60);

fn system32(name: &str) -> PathBuf {
    let root =
        std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from("C:\\Windows"), PathBuf::from);
    root.join("System32").join(name)
}

/// A Python that can make a venv: the `py` launcher, else `python` (but not the
/// Microsoft Store placeholder, which opens the Store instead of running).
fn python() -> Option<PathBuf> {
    crate::tools::which("py").or_else(|| {
        crate::tools::which("python").filter(|p| {
            !p.to_string_lossy()
                .to_lowercase()
                .contains("\\microsoft\\windowsapps\\")
        })
    })
}

/// Why the Install button can't work on this machine, if it can't.
pub fn blocked(def: &ScannerDef) -> Option<String> {
    match def.method {
        Method::Release { .. } => {
            if !system32("curl.exe").is_file() || !system32("tar.exe").is_file() {
                Some("Needs Windows 10 (1803) or newer, which include curl and tar.".into())
            } else {
                None
            }
        }
        Method::PythonVenv { .. } => python().is_none().then(|| {
            "Needs Python 3.9 or newer: install it from python.org, then check again.".into()
        }),
        Method::Cargo { .. } => crate::tools::which("cargo")
            .is_none()
            .then(|| "Needs Rust's cargo: install Rust from rustup.rs, then check again.".into()),
    }
}

fn version_file(locator: &Locator, def: &ScannerDef) -> PathBuf {
    locator.bin_dir().join(format!("{}.version", def.program))
}

pub fn installed_version(locator: &Locator, def: &ScannerDef) -> Option<String> {
    fs::read_to_string(version_file(locator, def))
        .ok()
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

/// What an install reports while it runs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Update<'a> {
    /// A step, in words ("Downloading Trivy v0.60.0 (58 MB)").
    Stage(&'a str),
    /// A line for the install log: a command DevDock runs, or its output.
    Line(&'a str),
    /// How far the current step is, 0 to 1; `None` when it can't be told.
    Progress(Option<f64>),
}

struct Ctx<'a, F: Fn(Update<'_>)> {
    tree: &'a dyn ProcessTree,
    cancel: &'a AtomicBool,
    report: F,
    cwd: PathBuf,
}

/// How a step's output is handled.
#[derive(Clone, Copy)]
enum Output<'a> {
    /// Not shown (JSON from GitHub's API).
    Hidden,
    /// Each line goes to the install log (pip, cargo).
    Logged,
    /// A download to this file of this size: progress from the file's size.
    Download(&'a Path, u64),
}

impl<F: Fn(Update<'_>)> Ctx<'_, F> {
    fn stage(&self, text: &str) {
        (self.report)(Update::Stage(text));
        (self.report)(Update::Progress(None));
    }

    fn line(&self, text: &str) {
        (self.report)(Update::Line(text));
    }

    fn run(
        &self,
        line: &str,
        timeout: Duration,
        what: &str,
        output: Output<'_>,
    ) -> Result<String, String> {
        self.line(&format!("> {line}"));
        let last = Cell::new(-1.0_f64);
        let mut on_line = |l: &str| {
            if matches!(output, Output::Logged) {
                self.line(l);
            }
        };
        let mut tick = || {
            if let Output::Download(file, size) = output
                && size > 0
            {
                let got = fs::metadata(file).map(|m| m.len()).unwrap_or(0);
                let frac = (got as f64 / size as f64).min(1.0);
                if frac - last.get() >= 0.01 {
                    last.set(frac);
                    (self.report)(Update::Progress(Some(frac)));
                }
            }
        };
        let watch = Watch {
            line: &mut on_line,
            tick: &mut tick,
        };
        let result = exec_watched(
            self.tree,
            &self.cwd,
            line,
            &[],
            timeout,
            self.cancel,
            Some(watch),
        );
        match result {
            Ok(c) if c.code == Some(0) => Ok(c.stdout),
            Ok(c) => {
                // Hidden output may hold the reason; show its tail in the log.
                if !matches!(output, Output::Logged) {
                    for l in c.stderr.lines().filter(|l| !l.trim().is_empty()).take(20) {
                        self.line(l);
                    }
                }
                Err(format!(
                    "{what} failed: {}",
                    last_line(&c.stderr)
                        .or_else(|| last_line(&c.stdout))
                        .unwrap_or("no details")
                ))
            }
            Err(ExecError::Cancelled) => Err(CANCELLED.into()),
            Err(ExecError::TimedOut) => Err(format!("{what} took too long and was stopped.")),
            Err(ExecError::Failed(e)) => Err(format!("{what} couldn't start: {e}")),
        }
    }
}

/// The error text of a cancelled install.
pub const CANCELLED: &str = "Cancelled.";

/// Installs `def`, telling `report` each step, log line and progress. Returns
/// the installed executable.
pub fn install(
    def: &ScannerDef,
    locator: &Locator,
    tree: &dyn ProcessTree,
    cancel: &AtomicBool,
    report: impl Fn(Update<'_>),
) -> Result<PathBuf, String> {
    if let Some(why) = blocked(def) {
        return Err(why);
    }
    fs::create_dir_all(&locator.tools_dir)
        .map_err(|e| format!("Couldn't create {}: {e}", locator.tools_dir.display()))?;
    let ctx = Ctx {
        tree,
        cancel,
        report,
        cwd: locator.tools_dir.clone(),
    };
    match def.method {
        Method::Release { repo } => install_release(def, repo, locator, &ctx),
        Method::PythonVenv { package } => install_venv(def, package, locator, &ctx),
        Method::Cargo { krate } => install_cargo(def, krate, &ctx),
    }
}

/// Removes a scanner DevDock installed. Ones installed some other way are left alone.
pub fn uninstall(def: &ScannerDef, locator: &Locator) -> Result<(), String> {
    let result = match def.method {
        Method::Release { .. } => {
            let _ = fs::remove_file(version_file(locator, def));
            match locator.managed_path(def) {
                Some(p) if p.is_file() => fs::remove_file(p),
                _ => Ok(()),
            }
        }
        Method::PythonVenv { .. } => {
            let _ = fs::remove_file(version_file(locator, def));
            match fs::remove_dir_all(locator.venv_dir(def)) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            }
        }
        Method::Cargo { .. } => {
            return Err(format!(
                "{} was installed with cargo; remove it with: cargo uninstall {}",
                def.name, def.program
            ));
        }
    };
    result.map_err(|e| format!("Couldn't remove {}: {e}", def.name))
}

// --- GitHub releases ----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
    /// `sha256:<hex>` when GitHub reports it.
    pub digest: Option<String>,
}

pub fn assets(release: &Value) -> Vec<Asset> {
    release
        .get("assets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|a| {
            Some(Asset {
                name: a.get("name")?.as_str()?.to_owned(),
                url: a.get("browser_download_url")?.as_str()?.to_owned(),
                size: a.get("size").and_then(Value::as_u64).unwrap_or(0),
                digest: a.get("digest").and_then(Value::as_str).map(str::to_owned),
            })
        })
        .collect()
}

/// The Windows build for this CPU: a `.zip` or a bare `.exe` whose name says
/// windows and the architecture. Signatures, SBOMs and checksums are skipped.
pub fn pick_asset<'a>(assets: &'a [Asset], arch: &str) -> Option<&'a Asset> {
    let tokens: &[&str] = match arch {
        "aarch64" => &["arm64", "aarch64"],
        _ => &["amd64", "x86_64", "x64", "64bit"],
    };
    let wrong: &[&str] = match arch {
        "aarch64" => &[],
        _ => &["arm64", "aarch64", "arm"],
    };
    assets.iter().find(|a| {
        let n = a.name.to_lowercase();
        (n.ends_with(".zip") || n.ends_with(".exe"))
            && n.contains("windows")
            && tokens.iter().any(|t| n.contains(t))
            && !wrong.iter().any(|t| n.contains(t))
            && !["sbom", ".sig", ".pem", "checksum", "sha256"]
                .iter()
                .any(|t| n.contains(t))
    })
}

fn checksums_asset(assets: &[Asset]) -> Option<&Asset> {
    assets.iter().find(|a| {
        let n = a.name.to_lowercase();
        (n.contains("checksums") || n.contains("sha256sums"))
            && ![".sig", ".pem", ".sbom", ".bundle"]
                .iter()
                .any(|t| n.ends_with(t))
    })
}

/// The hash for `name` in a `sha256sum`-style file (`<hex>  <name>` or `<hex> *<name>`).
pub fn checksum_for(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let file = parts.next()?.trim_start_matches('*');
        let file = file.rsplit('/').next().unwrap_or(file);
        (file == name && hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| hash.to_lowercase())
    })
}

pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut ctx = ring::digest::Context::new(&ring::digest::SHA256);
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        ctx.update(&buf[..n]);
    }
    Ok(ctx
        .finish()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Downloads go only to GitHub's release URLs for this repository.
fn trusted(url: &str, repo: &str) -> bool {
    url.starts_with(&format!("https://github.com/{repo}/releases/download/"))
        && !url.contains('"')
        && !url.contains('%')
}

fn find_file(dir: &Path, file_name: &str) -> Option<PathBuf> {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .flatten()
        .find(|e| e.file_type().is_file() && e.file_name().eq_ignore_ascii_case(file_name))
        .map(walkdir::DirEntry::into_path)
}

fn install_release<F: Fn(Update<'_>)>(
    def: &ScannerDef,
    repo: &str,
    locator: &Locator,
    ctx: &Ctx<'_, F>,
) -> Result<PathBuf, String> {
    let curl = quote(&system32("curl.exe").to_string_lossy());
    let tar = quote(&system32("tar.exe").to_string_lossy());

    ctx.stage("Finding the latest release");
    let api = format!(
        "{curl} -fsSL --retry 2 -H \"Accept: application/vnd.github+json\" -H \"User-Agent: DevDock\" https://api.github.com/repos/{repo}/releases/latest"
    );
    let text = ctx.run(
        &api,
        API_TIMEOUT,
        "Asking GitHub for the latest release",
        Output::Hidden,
    )?;
    let release: Value = serde_json::from_str(&text)
        .map_err(|_| "GitHub's answer about the latest release couldn't be read.".to_owned())?;
    let tag = release
        .get("tag_name")
        .and_then(Value::as_str)
        .unwrap_or("latest")
        .to_owned();
    let list = assets(&release);
    let asset = pick_asset(&list, std::env::consts::ARCH)
        .ok_or_else(|| format!("{} {tag} has no Windows build for this computer.", def.name))?
        .clone();
    if !trusted(&asset.url, repo) {
        return Err(format!(
            "Unexpected download address for {}: {}",
            def.name, asset.url
        ));
    }

    let staging = locator.tools_dir.join(format!(".download-{}", def.id));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|e| format!("Couldn't create a download folder: {e}"))?;
    let result = (|| {
        // The checksum: GitHub's own digest when it has one, else the project's file.
        let expected = match asset
            .digest
            .as_deref()
            .and_then(|d| d.strip_prefix("sha256:"))
        {
            Some(hex) => hex.to_lowercase(),
            None => {
                let sums = checksums_asset(&list).ok_or_else(|| {
                    format!(
                        "{} {tag} publishes no checksums, so DevDock won't install it.",
                        def.name
                    )
                })?;
                if !trusted(&sums.url, repo) {
                    return Err(format!("Unexpected checksum address: {}", sums.url));
                }
                let file = staging.join("checksums.txt");
                ctx.run(
                    &format!(
                        "{curl} -fsSL --retry 2 -o {} {}",
                        quote(&file.to_string_lossy()),
                        quote(&sums.url)
                    ),
                    API_TIMEOUT,
                    "Downloading the checksums",
                    Output::Hidden,
                )?;
                let text = fs::read_to_string(&file).unwrap_or_default();
                checksum_for(&text, &asset.name)
                    .ok_or_else(|| format!("The checksums file doesn't list {}.", asset.name))?
            }
        };

        ctx.stage(&format!(
            "Downloading {} {tag} ({:.0} MB)",
            def.name,
            (asset.size as f64 / 1_048_576.0).max(1.0)
        ));
        let file = staging.join(&asset.name);
        ctx.run(
            &format!(
                "{curl} -fL --retry 2 -s -o {} {}",
                quote(&file.to_string_lossy()),
                quote(&asset.url)
            ),
            DOWNLOAD_TIMEOUT,
            "The download",
            Output::Download(&file, asset.size),
        )?;
        ctx.line(&format!(
            "Downloaded {} ({} bytes)",
            asset.name,
            fs::metadata(&file).map(|m| m.len()).unwrap_or(0)
        ));

        ctx.stage("Checking the download");
        let actual = sha256_file(&file).map_err(|e| format!("Couldn't read the download: {e}"))?;
        ctx.line(&format!("SHA-256 {actual}"));
        ctx.line(&format!("expected {expected}"));
        if actual != expected {
            return Err(format!(
                "The download's SHA-256 doesn't match the one {} published, so it was thrown away.",
                def.name
            ));
        }

        let exe_name = format!("{}.exe", def.program);
        let exe = if asset.name.to_lowercase().ends_with(".zip") {
            ctx.stage("Unpacking");
            let out = staging.join("x");
            fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            ctx.run(
                &format!(
                    "{tar} -xf {} -C {}",
                    quote(&file.to_string_lossy()),
                    quote(&out.to_string_lossy())
                ),
                API_TIMEOUT,
                "Unpacking",
                Output::Logged,
            )?;
            find_file(&out, &exe_name)
                .ok_or_else(|| format!("The download has no {exe_name} in it."))?
        } else {
            file
        };

        let bin = locator.bin_dir();
        fs::create_dir_all(&bin).map_err(|e| e.to_string())?;
        let target = bin.join(&exe_name);
        let _ = fs::remove_file(&target);
        fs::rename(&exe, &target)
            .or_else(|_| fs::copy(&exe, &target).map(drop))
            .map_err(|e| format!("Couldn't put {exe_name} in place: {e}"))?;
        let _ = fs::write(version_file(locator, def), &tag);
        ctx.line(&format!("Installed {}", target.display()));
        Ok(target)
    })();
    let _ = fs::remove_dir_all(&staging);
    result
}

// --- Python and cargo ---------------------------------------------------------------

fn install_venv<F: Fn(Update<'_>)>(
    def: &ScannerDef,
    package: &str,
    locator: &Locator,
    ctx: &Ctx<'_, F>,
) -> Result<PathBuf, String> {
    let py = python().ok_or("Python was not found.")?;
    let venv = locator.venv_dir(def);
    ctx.stage("Creating a Python environment");
    let _ = fs::remove_dir_all(&venv);
    ctx.run(
        &format!(
            "{} -m venv {}",
            quote(&py.to_string_lossy()),
            quote(&venv.to_string_lossy())
        ),
        Duration::from_secs(5 * 60),
        "Creating the Python environment",
        Output::Logged,
    )?;
    let venv_py = venv.join("Scripts").join("python.exe");
    ctx.stage(&format!("Installing {package} with pip (a few minutes)"));
    let result = ctx.run(
        &format!(
            "{} -m pip install --upgrade --disable-pip-version-check --no-input {package}",
            quote(&venv_py.to_string_lossy())
        ),
        PIP_TIMEOUT,
        &format!("pip install {package}"),
        Output::Logged,
    );
    if let Err(e) = result {
        let _ = fs::remove_dir_all(&venv);
        return Err(e);
    }
    let exe = locator
        .managed_path(def)
        .filter(|p| p.is_file())
        .ok_or_else(|| format!("pip finished, but {}.exe isn't there.", def.program))?;
    // pip prints "Successfully installed semgrep-1.2.3 ..."; keep the version.
    let shown = ctx
        .run(
            &format!(
                "{} -m pip show {package}",
                quote(&venv_py.to_string_lossy())
            ),
            Duration::from_secs(60),
            "pip show",
            Output::Hidden,
        )
        .ok()
        .and_then(|t| {
            t.lines()
                .find_map(|l| l.strip_prefix("Version:").map(|v| v.trim().to_owned()))
        });
    ctx.line(&format!("Installed {}", exe.display()));
    if let Some(v) = shown {
        let _ = fs::create_dir_all(locator.bin_dir());
        let _ = fs::write(version_file(locator, def), v);
    }
    Ok(exe)
}

fn install_cargo<F: Fn(Update<'_>)>(
    def: &ScannerDef,
    krate: &str,
    ctx: &Ctx<'_, F>,
) -> Result<PathBuf, String> {
    let cargo = crate::tools::which("cargo").ok_or("cargo was not found.")?;
    ctx.stage(&format!("Building {krate} with cargo (several minutes)"));
    ctx.run(
        &format!(
            "{} install {krate} --locked",
            quote(&cargo.to_string_lossy())
        ),
        CARGO_TIMEOUT,
        &format!("cargo install {krate}"),
        Output::Logged,
    )?;
    cargo_bin(def.program)
        .filter(|p| p.is_file())
        .or_else(|| crate::tools::which(def.program))
        .ok_or_else(|| format!("cargo finished, but {} isn't on PATH.", def.program))
}

/// For callers that only know the id.
pub fn def(id: &str) -> Option<&'static ScannerDef> {
    scanners::by_id(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str) -> Asset {
        Asset {
            name: name.into(),
            url: format!("https://github.com/o/r/releases/download/v1/{name}"),
            size: 1,
            digest: None,
        }
    }

    #[test]
    fn picks_the_windows_build_for_each_project() {
        let grype: Vec<Asset> = [
            "grype_0.121.0_checksums.txt",
            "grype_0.121.0_darwin_arm64.tar.gz",
            "grype_0.121.0_linux_amd64.tar.gz",
            "grype_0.121.0_windows_amd64.zip",
            "grype_0.121.0_windows_amd64.zip.sbom",
            "grype_0.121.0_windows_arm64.zip",
        ]
        .map(asset)
        .to_vec();
        assert_eq!(
            pick_asset(&grype, "x86_64").unwrap().name,
            "grype_0.121.0_windows_amd64.zip"
        );
        assert_eq!(
            pick_asset(&grype, "aarch64").unwrap().name,
            "grype_0.121.0_windows_arm64.zip"
        );

        let trivy: Vec<Asset> = [
            "trivy_0.60.0_Linux-64bit.tar.gz",
            "trivy_0.60.0_windows-64bit.zip",
            "trivy_0.60.0_windows-64bit.zip.pem",
        ]
        .map(asset)
        .to_vec();
        assert_eq!(
            pick_asset(&trivy, "x86_64").unwrap().name,
            "trivy_0.60.0_windows-64bit.zip"
        );

        let gitleaks: Vec<Asset> = [
            "gitleaks_8.28.0_windows_x32.zip",
            "gitleaks_8.28.0_windows_x64.zip",
            "gitleaks_8.28.0_windows_armv7.zip",
        ]
        .map(asset)
        .to_vec();
        assert_eq!(
            pick_asset(&gitleaks, "x86_64").unwrap().name,
            "gitleaks_8.28.0_windows_x64.zip"
        );

        let osv: Vec<Asset> = [
            "osv-scanner_SHA256SUMS",
            "osv-scanner_linux_amd64",
            "osv-scanner_windows_amd64.exe",
            "osv-scanner_windows_arm64.exe",
        ]
        .map(asset)
        .to_vec();
        assert_eq!(
            pick_asset(&osv, "x86_64").unwrap().name,
            "osv-scanner_windows_amd64.exe"
        );
        assert_eq!(
            checksums_asset(&osv).unwrap().name,
            "osv-scanner_SHA256SUMS"
        );
    }

    #[test]
    fn checksum_lines() {
        let hex = "a".repeat(64);
        let text = format!(
            "{hex}  grype_windows_amd64.zip\n{}  other.zip\n",
            "b".repeat(64)
        );
        assert_eq!(
            checksum_for(&text, "grype_windows_amd64.zip"),
            Some(hex.clone())
        );
        assert_eq!(
            checksum_for(&format!("{hex} *./dist/x.exe"), "x.exe"),
            Some(hex)
        );
        assert_eq!(checksum_for("short  x.exe", "x.exe"), None);
        assert_eq!(checksum_for(&text, "missing.zip"), None);
    }

    #[test]
    fn sha256_of_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("x");
        fs::write(&f, "abc").unwrap();
        assert_eq!(
            sha256_file(&f).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn only_this_repositorys_release_urls() {
        assert!(trusted(
            "https://github.com/anchore/grype/releases/download/v1/grype.zip",
            "anchore/grype"
        ));
        assert!(!trusted(
            "https://evil.example/anchore/grype/releases/download/v1/x.zip",
            "anchore/grype"
        ));
        assert!(!trusted(
            "https://github.com/other/grype/releases/download/v1/x.zip",
            "anchore/grype"
        ));
        assert!(!trusted(
            "http://github.com/anchore/grype/releases/download/v1/x.zip",
            "anchore/grype"
        ));
    }

    #[test]
    fn uninstall_only_touches_managed_copies() {
        let dir = tempfile::tempdir().unwrap();
        let loc = Locator::new(dir.path());
        fs::create_dir_all(loc.bin_dir()).unwrap();
        fs::write(loc.bin_dir().join("trivy.exe"), "").unwrap();
        fs::write(loc.bin_dir().join("trivy.version"), "v0.60.0\n").unwrap();
        let trivy = def("trivy").unwrap();
        assert_eq!(installed_version(&loc, trivy).as_deref(), Some("v0.60.0"));
        uninstall(trivy, &loc).unwrap();
        assert!(!loc.bin_dir().join("trivy.exe").exists());
        assert!(installed_version(&loc, trivy).is_none());
        assert!(uninstall(def("cargo-audit").unwrap(), &loc).is_err());
    }
}
