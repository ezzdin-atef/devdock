//! Coding agents and terminals (M6). `plan` turns "Claude Code in Windows Terminal,
//! in this folder" into an exact program + arguments. It is pure, so every
//! terminal's command line is unit-tested without opening a window.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::tools::ToolScan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentId {
    #[default]
    Claude,
    Codex,
    Opencode,
}

pub const AGENTS: [AgentId; 3] = [AgentId::Claude, AgentId::Codex, AgentId::Opencode];

impl AgentId {
    /// The executable DevDock looks for on PATH and runs.
    pub fn command(self) -> &'static str {
        match self {
            AgentId::Claude => "claude",
            AgentId::Codex => "codex",
            AgentId::Opencode => "opencode",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            AgentId::Claude => "Claude Code",
            AgentId::Codex => "Codex",
            AgentId::Opencode => "OpenCode",
        }
    }

    /// One common way to install it; shown with the "not installed" state.
    pub fn install_hint(self) -> &'static str {
        match self {
            AgentId::Claude => "npm install -g @anthropic-ai/claude-code",
            AgentId::Codex => "npm install -g @openai/codex",
            AgentId::Opencode => "npm install -g opencode-ai",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminalKind {
    #[default]
    WindowsTerminal,
    PowerShell,
    GitBash,
    Cmd,
}

pub const TERMINALS: [TerminalKind; 4] = [
    TerminalKind::WindowsTerminal,
    TerminalKind::PowerShell,
    TerminalKind::GitBash,
    TerminalKind::Cmd,
];

impl TerminalKind {
    pub fn name(self) -> &'static str {
        match self {
            TerminalKind::WindowsTerminal => "Windows Terminal",
            TerminalKind::PowerShell => "PowerShell",
            TerminalKind::GitBash => "Git Bash",
            TerminalKind::Cmd => "Command Prompt",
        }
    }

    pub fn available(self, tools: &ToolScan) -> bool {
        match self {
            TerminalKind::WindowsTerminal => tools.path_of("wt").is_some(),
            TerminalKind::PowerShell => {
                tools.path_of("pwsh").is_some() || tools.path_of("powershell").is_some()
            }
            TerminalKind::GitBash => tools.git_bash.is_some(),
            TerminalKind::Cmd => true,
        }
    }
}

/// What the user asked for (T6.2). `program: None` just opens a terminal there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchSpec {
    pub terminal: TerminalKind,
    pub cwd: PathBuf,
    pub program: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arg {
    /// Quoted by `std` with the usual Windows rules.
    Plain(String),
    /// Passed through untouched (for `cmd /k`, which has its own quoting).
    Raw(String),
}

/// Exactly what gets spawned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub program: PathBuf,
    pub args: Vec<Arg>,
    pub cwd: PathBuf,
    /// Console programs need their own window; Windows Terminal and Git Bash make one.
    pub new_console: bool,
    /// Extra environment for the terminal (on top of DevDock's own).
    pub env: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlanError {
    #[error("{} wasn't found", .0.name())]
    TerminalMissing(TerminalKind),
    #[error("{0} isn't a program name DevDock will pass to a shell")]
    UnsafeProgram(String),
}

/// Program names come from DevDock's own agent list, but are still checked: they
/// end up inside shell command lines, where `;`, `&`, `|` or quotes would mean more.
fn safe_program(p: &str) -> bool {
    !p.is_empty()
        && p.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '/' | '='))
}

fn powershell(tools: &ToolScan) -> Option<PathBuf> {
    tools
        .path_of("pwsh")
        .or_else(|| tools.path_of("powershell"))
        .map(Path::to_path_buf)
}

pub fn plan(spec: &LaunchSpec, tools: &ToolScan) -> Result<LaunchPlan, PlanError> {
    if let Some(p) = &spec.program
        && !safe_program(p)
    {
        return Err(PlanError::UnsafeProgram(p.clone()));
    }
    let missing = || PlanError::TerminalMissing(spec.terminal);
    let cwd = spec.cwd.clone();
    let plain = |s: &str| Arg::Plain(s.to_owned());

    let plan = match spec.terminal {
        // wt -d <cwd> [pwsh -NoExit -Command <program>]
        TerminalKind::WindowsTerminal => {
            let wt = tools.path_of("wt").ok_or_else(missing)?.to_path_buf();
            let mut args = vec![plain("-d"), Arg::Plain(cwd.display().to_string())];
            if let Some(program) = &spec.program {
                let shell = powershell(tools).ok_or_else(missing)?;
                args.extend([
                    Arg::Plain(shell.display().to_string()),
                    plain("-NoExit"),
                    plain("-Command"),
                    plain(program),
                ]);
            }
            LaunchPlan {
                program: wt,
                args,
                cwd,
                new_console: false,
                env: Vec::new(),
            }
        }
        // pwsh -NoExit [-Command <program>], in its own console window
        TerminalKind::PowerShell => {
            let shell = powershell(tools).ok_or_else(missing)?;
            let mut args = vec![plain("-NoExit")];
            if let Some(program) = &spec.program {
                args.extend([plain("-Command"), plain(program)]);
            }
            LaunchPlan {
                program: shell,
                args,
                cwd,
                new_console: true,
                env: Vec::new(),
            }
        }
        // git-bash.exe --cd=<cwd> [-c "<program>; exec bash"]: stays open afterwards.
        // Git Bash starts a login shell, and /etc/profile changes to $HOME unless
        // CHERE_INVOKING is set: without it the agent would start in the wrong folder.
        TerminalKind::GitBash => {
            let bash = tools.git_bash.clone().ok_or_else(missing)?;
            let mut args = vec![Arg::Plain(format!("--cd={}", cwd.display()))];
            if let Some(program) = &spec.program {
                args.extend([plain("-c"), Arg::Plain(format!("{program}; exec bash"))]);
            }
            LaunchPlan {
                program: bash,
                args,
                cwd,
                new_console: false,
                env: vec![("CHERE_INVOKING".into(), "1".into())],
            }
        }
        // cmd /k <program>
        TerminalKind::Cmd => LaunchPlan {
            program: tools
                .path_of("cmd")
                .map_or_else(|| PathBuf::from("cmd.exe"), Path::to_path_buf),
            args: spec
                .program
                .as_ref()
                .map(|p| vec![Arg::Raw(format!("/k {p}"))])
                .unwrap_or_default(),
            cwd,
            new_console: true,
            env: Vec::new(),
        },
    };
    Ok(plan)
}

/// One launch, kept per project for the "last launched" line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRecord {
    pub agent: Option<AgentId>,
    pub terminal: TerminalKind,
    pub subdir: Option<String>,
    pub at: u64,
}

pub const KEEP_LAUNCHES: usize = 10;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolFound;

    fn tools(found: &[(&str, &str)]) -> ToolScan {
        ToolScan {
            tools: found
                .iter()
                .map(|(id, path)| ToolFound {
                    id: (*id).into(),
                    name: (*id).into(),
                    path: Some(PathBuf::from(path)),
                })
                .collect(),
            git_bash: Some(PathBuf::from(r"C:\Git\git-bash.exe")),
            checked_at: 0,
        }
    }

    fn all() -> ToolScan {
        tools(&[
            ("wt", r"C:\Apps\wt.exe"),
            ("pwsh", r"C:\PS7\pwsh.exe"),
            ("powershell", r"C:\Win\powershell.exe"),
            ("cmd", r"C:\Win\cmd.exe"),
        ])
    }

    fn spec(t: TerminalKind, program: Option<&str>) -> LaunchSpec {
        LaunchSpec {
            terminal: t,
            cwd: PathBuf::from(r"D:\dev\my shop"),
            program: program.map(str::to_owned),
        }
    }

    fn plain(args: &[Arg]) -> Vec<String> {
        args.iter()
            .map(|a| match a {
                Arg::Plain(s) | Arg::Raw(s) => s.clone(),
            })
            .collect()
    }

    #[test]
    fn windows_terminal_runs_the_agent_in_powershell_in_the_folder() {
        let p = plan(&spec(TerminalKind::WindowsTerminal, Some("claude")), &all()).unwrap();
        assert_eq!(p.program, PathBuf::from(r"C:\Apps\wt.exe"));
        assert_eq!(
            plain(&p.args),
            vec![
                "-d",
                r"D:\dev\my shop",
                r"C:\PS7\pwsh.exe",
                "-NoExit",
                "-Command",
                "claude"
            ]
        );
        assert!(!p.new_console);
    }

    #[test]
    fn plain_terminal_has_no_command() {
        let p = plan(&spec(TerminalKind::WindowsTerminal, None), &all()).unwrap();
        assert_eq!(plain(&p.args), vec!["-d", r"D:\dev\my shop"]);
    }

    #[test]
    fn powershell_falls_back_to_windows_powershell() {
        let t = tools(&[("powershell", r"C:\Win\powershell.exe")]);
        let p = plan(&spec(TerminalKind::PowerShell, Some("codex")), &t).unwrap();
        assert_eq!(p.program, PathBuf::from(r"C:\Win\powershell.exe"));
        assert_eq!(plain(&p.args), vec!["-NoExit", "-Command", "codex"]);
        assert!(p.new_console);
        assert_eq!(p.cwd, PathBuf::from(r"D:\dev\my shop"));
    }

    #[test]
    fn git_bash_keeps_the_shell_open_after_the_agent() {
        let p = plan(&spec(TerminalKind::GitBash, Some("opencode")), &all()).unwrap();
        assert_eq!(p.program, PathBuf::from(r"C:\Git\git-bash.exe"));
        assert_eq!(
            plain(&p.args),
            vec![r"--cd=D:\dev\my shop", "-c", "opencode; exec bash"]
        );
        assert_eq!(p.env, vec![("CHERE_INVOKING".to_string(), "1".to_string())]);
    }

    #[test]
    fn cmd_uses_a_raw_k_switch() {
        let p = plan(&spec(TerminalKind::Cmd, Some("claude")), &all()).unwrap();
        assert_eq!(p.args, vec![Arg::Raw("/k claude".into())]);
        assert!(p.new_console);
    }

    #[test]
    fn missing_terminal_is_an_error() {
        let none = tools(&[]);
        assert_eq!(
            plan(&spec(TerminalKind::WindowsTerminal, None), &none),
            Err(PlanError::TerminalMissing(TerminalKind::WindowsTerminal))
        );
        let mut no_bash = all();
        no_bash.git_bash = None;
        assert!(plan(&spec(TerminalKind::GitBash, None), &no_bash).is_err());
        assert!(!TerminalKind::GitBash.available(&no_bash));
        assert!(TerminalKind::Cmd.available(&none));
    }

    #[test]
    fn shell_metacharacters_are_refused() {
        for bad in ["claude & del x", "a;b", "x|y", "\"q\"", ""] {
            assert!(matches!(
                plan(&spec(TerminalKind::Cmd, Some(bad)), &all()),
                Err(PlanError::UnsafeProgram(_))
            ));
        }
        assert!(plan(&spec(TerminalKind::Cmd, Some("claude --resume")), &all()).is_ok());
    }

    #[test]
    fn agent_ids_round_trip_as_camel_case() {
        assert_eq!(serde_json::to_value(AgentId::Opencode).unwrap(), "opencode");
        assert_eq!(
            serde_json::to_value(TerminalKind::GitBash).unwrap(),
            "gitBash"
        );
        let t: TerminalKind = serde_json::from_str("\"windowsTerminal\"").unwrap();
        assert_eq!(t, TerminalKind::WindowsTerminal);
    }
}
