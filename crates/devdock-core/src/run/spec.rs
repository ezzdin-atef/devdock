//! What can be run in a project (T4.1): `package.json` scripts, a few detected
//! defaults (`cargo run`, `docker compose up`) and custom commands the user saved.

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A command the user saved for one project. Persisted inside `Project`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomCommand {
    pub id: String,
    pub name: String,
    pub command_line: String,
    /// Relative folder inside the project to run in; `None` = project root.
    #[serde(default)]
    pub subdir: Option<String>,
}

/// Stable within a project: `script:dev`, `detected:cargo-run`, `custom:<id>`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommandId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
}

impl PackageManager {
    pub fn program(self) -> &'static str {
        match self {
            PackageManager::Npm => "npm",
            PackageManager::Pnpm => "pnpm",
            PackageManager::Yarn => "yarn",
            PackageManager::Bun => "bun",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CommandSource {
    /// From `package.json`; `body` is the script text, shown as a hint.
    Script {
        body: String,
    },
    Detected,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunCommand {
    pub id: CommandId,
    pub name: String,
    /// Exactly what is passed to `cmd /C`.
    pub command_line: String,
    pub cwd: PathBuf,
    pub source: CommandSource,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Discovered {
    pub commands: Vec<RunCommand>,
    /// e.g. "package.json could not be read"; the rest of the list is still valid.
    pub warning: Option<String>,
}

/// Picks the package manager from the lockfile, falling back to npm.
pub fn package_manager(dir: &Path) -> PackageManager {
    let has = |f: &str| dir.join(f).is_file();
    if has("pnpm-lock.yaml") {
        PackageManager::Pnpm
    } else if has("yarn.lock") {
        PackageManager::Yarn
    } else if has("bun.lockb") || has("bun.lock") {
        PackageManager::Bun
    } else {
        PackageManager::Npm
    }
}

/// Script names are passed through `cmd`, so anything unusual is quoted.
fn quote_script(name: &str) -> String {
    let plain = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '-' | '_' | '.'));
    if plain {
        name.to_owned()
    } else {
        format!("\"{}\"", name.replace('"', ""))
    }
}

pub fn discover(project_dir: &Path, custom: &[CustomCommand]) -> Discovered {
    let mut out = Discovered::default();

    let package = project_dir.join("package.json");
    if package.is_file() {
        match read_scripts(&package) {
            Ok(scripts) => {
                let pm = package_manager(project_dir);
                out.commands
                    .extend(scripts.into_iter().map(|(name, body)| RunCommand {
                        id: CommandId(format!("script:{name}")),
                        command_line: format!("{} run {}", pm.program(), quote_script(&name)),
                        cwd: project_dir.to_path_buf(),
                        source: CommandSource::Script { body },
                        name,
                    }));
            }
            Err(msg) => out.warning = Some(msg),
        }
    }

    let detected = [
        ("Cargo.toml", "cargo-run", "cargo run", "cargo run"),
        (
            "docker-compose.yml",
            "compose-up",
            "compose up",
            "docker compose up",
        ),
        (
            "compose.yaml",
            "compose-up",
            "compose up",
            "docker compose up",
        ),
    ];
    for (marker, id, name, line) in detected {
        let id = CommandId(format!("detected:{id}"));
        if project_dir.join(marker).is_file() && !out.commands.iter().any(|c| c.id == id) {
            out.commands.push(RunCommand {
                id,
                name: name.into(),
                command_line: line.into(),
                cwd: project_dir.to_path_buf(),
                source: CommandSource::Detected,
            });
        }
    }

    out.commands.extend(custom.iter().map(|c| {
        RunCommand {
            id: CommandId(format!("custom:{}", c.id)),
            name: c.name.clone(),
            command_line: c.command_line.clone(),
            cwd: c
                .subdir
                .as_deref()
                .map_or_else(|| project_dir.to_path_buf(), |s| project_dir.join(s)),
            source: CommandSource::Custom,
        }
    }));
    out
}

/// Script names and bodies in file order.
fn read_scripts(package: &Path) -> Result<Vec<(String, String)>, String> {
    let text =
        fs::read_to_string(package).map_err(|e| format!("package.json could not be read: {e}"))?;
    let json: Value = serde_json::from_str(&text)
        .map_err(|e| format!("package.json is not valid JSON ({e}), so its scripts are hidden"))?;
    Ok(json
        .get("scripts")
        .and_then(Value::as_object)
        .map(|scripts| {
            scripts
                .iter()
                .map(|(name, body)| (name.clone(), body.as_str().unwrap_or_default().to_owned()))
                .collect()
        })
        .unwrap_or_default())
}

/// Checks a custom command before saving it. Returns the cleaned subdir.
pub fn validate_custom(
    project_dir: &Path,
    name: &str,
    command_line: &str,
    subdir: Option<&str>,
) -> Result<Option<String>, String> {
    if name.trim().is_empty() {
        return Err("Give the command a name.".into());
    }
    if command_line.trim().is_empty() {
        return Err("Type the command to run, for example `cargo run`.".into());
    }
    let Some(sub) = subdir.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let rel = Path::new(sub);
    let escapes = rel.is_absolute()
        || rel
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::Prefix(_)));
    if escapes {
        return Err(format!("{sub} must be a folder inside the project."));
    }
    if !project_dir.join(rel).is_dir() {
        return Err(format!("{sub} doesn't exist inside the project."));
    }
    Ok(Some(sub.trim_matches(['\\', '/']).to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_with(files: &[(&str, &str)]) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for (name, body) in files {
            let p = d.path().join(name);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, body).unwrap();
        }
        d
    }

    fn lines(d: &Discovered) -> Vec<(&str, &str)> {
        d.commands
            .iter()
            .map(|c| (c.id.0.as_str(), c.command_line.as_str()))
            .collect()
    }

    #[test]
    fn scripts_keep_file_order_and_use_the_lockfile_manager() {
        let d = dir_with(&[
            (
                "package.json",
                r#"{"scripts":{"dev":"next dev","build":"next build","test:unit":"vitest"}}"#,
            ),
            ("pnpm-lock.yaml", ""),
        ]);
        let found = discover(d.path(), &[]);
        assert_eq!(
            lines(&found),
            vec![
                ("script:dev", "pnpm run dev"),
                ("script:build", "pnpm run build"),
                ("script:test:unit", "pnpm run test:unit"),
            ]
        );
        assert!(
            matches!(&found.commands[0].source, CommandSource::Script { body } if body == "next dev")
        );
    }

    #[test]
    fn npm_is_the_default_manager() {
        let d = dir_with(&[("package.json", r#"{"scripts":{"start":"node ."}}"#)]);
        assert_eq!(
            lines(&discover(d.path(), &[])),
            vec![("script:start", "npm run start")]
        );
    }

    #[test]
    fn odd_script_names_are_quoted() {
        assert_eq!(quote_script("dev"), "dev");
        assert_eq!(quote_script("dev & del"), "\"dev & del\"");
    }

    #[test]
    fn broken_package_json_warns_but_keeps_other_commands() {
        let d = dir_with(&[("package.json", "{ nope"), ("Cargo.toml", "")]);
        let found = discover(d.path(), &[]);
        assert!(found.warning.as_deref().unwrap().contains("not valid JSON"));
        assert_eq!(lines(&found), vec![("detected:cargo-run", "cargo run")]);
    }

    #[test]
    fn compose_is_detected_once() {
        let d = dir_with(&[("docker-compose.yml", ""), ("compose.yaml", "")]);
        assert_eq!(
            lines(&discover(d.path(), &[])),
            vec![("detected:compose-up", "docker compose up")]
        );
    }

    #[test]
    fn custom_commands_run_in_their_subfolder() {
        let d = dir_with(&[("api/main.rs", "")]);
        let custom = [CustomCommand {
            id: "abc".into(),
            name: "api".into(),
            command_line: "cargo run".into(),
            subdir: Some("api".into()),
        }];
        let found = discover(d.path(), &custom);
        assert_eq!(found.commands[0].id.0, "custom:abc");
        assert_eq!(found.commands[0].cwd, d.path().join("api"));
    }

    #[test]
    fn custom_validation() {
        let d = dir_with(&[("web/x", "")]);
        assert!(validate_custom(d.path(), "", "x", None).is_err());
        assert!(validate_custom(d.path(), "a", " ", None).is_err());
        assert!(validate_custom(d.path(), "a", "x", Some("..\\other")).is_err());
        assert!(validate_custom(d.path(), "a", "x", Some("C:\\abs")).is_err());
        assert!(validate_custom(d.path(), "a", "x", Some("missing")).is_err());
        assert_eq!(
            validate_custom(d.path(), "a", "x", Some("web\\")).unwrap(),
            Some("web".into())
        );
        assert_eq!(
            validate_custom(d.path(), "a", "x", Some("  ")).unwrap(),
            None
        );
    }
}
