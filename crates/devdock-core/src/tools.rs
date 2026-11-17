//! Finding executables on PATH the way Windows does (PATH + PATHEXT), so `code`
//! resolves to `code.cmd`, which `std::process::Command` would not find on its own.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// Every executable DevDock looks for: (id/command, display name).
pub const TOOLS: &[(&str, &str)] = &[
    ("explorer", "Explorer"),
    ("git", "Git"),
    ("code", "VS Code"),
    ("cursor", "Cursor"),
    ("code-insiders", "VS Code Insiders"),
    ("windsurf", "Windsurf"),
    ("zed", "Zed"),
    ("wt", "Windows Terminal"),
    ("pwsh", "PowerShell 7"),
    ("powershell", "Windows PowerShell"),
    ("cmd", "Command Prompt"),
    ("docker", "Docker"),
    ("claude", "Claude Code"),
    ("codex", "Codex"),
    ("opencode", "OpenCode"),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolFound {
    pub id: String,
    pub name: String,
    pub path: Option<PathBuf>,
}

/// Result of one discovery pass (T6.1). Cached by the app; "Re-check" runs it again.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolScan {
    pub tools: Vec<ToolFound>,
    /// `git-bash.exe` is not on PATH; it's found next to Git.
    pub git_bash: Option<PathBuf>,
    pub checked_at: u64,
}

impl ToolScan {
    pub fn detect() -> Self {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let pathext = std::env::var_os("PATHEXT").unwrap_or_else(|| DEFAULT_PATHEXT.into());
        let mut scan = Self::scan_with(&path, &pathext, exists_as_file);
        let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        scan.add_wt_fallback(local.as_deref(), exists_as_file);
        scan.checked_at = crate::time::now_ms();
        scan
    }

    /// Windows Terminal installs `wt.exe` as an app alias in
    /// `%LOCALAPPDATA%\Microsoft\WindowsApps`, which isn't always on PATH.
    pub fn add_wt_fallback(
        &mut self,
        local_app_data: Option<&Path>,
        is_file: impl Fn(&Path) -> bool,
    ) {
        let Some(slot) = self.tools.iter_mut().find(|t| t.id == "wt") else {
            return;
        };
        if slot.path.is_none() {
            slot.path = local_app_data
                .map(|l| l.join("Microsoft").join("WindowsApps").join("wt.exe"))
                .filter(|p| is_file(p));
        }
    }

    /// Pure version of [`detect`]: the file check is injected so tests use a fake PATH.
    pub fn scan_with(path: &OsStr, pathext: &OsStr, is_file: impl Fn(&Path) -> bool) -> Self {
        let tools: Vec<ToolFound> = TOOLS
            .iter()
            .map(|(id, name)| ToolFound {
                id: (*id).to_owned(),
                name: (*name).to_owned(),
                path: find_on_path(id, path, pathext, &is_file),
            })
            .collect();
        let git = tools
            .iter()
            .find(|t| t.id == "git")
            .and_then(|t| t.path.clone());
        Self {
            git_bash: git_bash_near(git.as_deref(), &is_file),
            tools,
            checked_at: 0,
        }
    }

    pub fn path_of(&self, id: &str) -> Option<&Path> {
        self.tools.iter().find(|t| t.id == id)?.path.as_deref()
    }
}

const DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD";

/// `Path::is_file` follows reparse points, and Windows "app execution aliases" (how
/// `wt.exe` is installed) can't be followed, so they look missing. Accept anything
/// that exists and isn't a directory.
fn exists_as_file(p: &Path) -> bool {
    p.is_file() || std::fs::symlink_metadata(p).is_ok_and(|m| !m.is_dir())
}

/// Git for Windows puts `git.exe` in a `cmd` (or `bin`) folder under its install
/// root, and `git-bash.exe` in the root itself; else try the default install folder.
fn git_bash_near(git: Option<&Path>, is_file: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    let near = git
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(|root| root.join("git-bash.exe"));
    near.into_iter()
        .chain([PathBuf::from(r"C:\Program Files\Git\git-bash.exe")])
        .find(|p| is_file(p))
}

/// Resolve `name` using the real `PATH` and `PATHEXT`.
pub fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let pathext = std::env::var_os("PATHEXT").unwrap_or_else(|| DEFAULT_PATHEXT.into());
    find_on_path(name, &path, &pathext, &exists_as_file)
}

/// Pure lookup used by [`which`]; `is_file` is injected so tests need no real files.
///
/// For each directory in `path` (in order), try `name` with each extension in `pathext`,
/// then `name` as given if it already has an extension. First hit wins.
pub fn find_on_path(
    name: &str,
    path: &OsStr,
    pathext: &OsStr,
    is_file: &impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let exts: Vec<String> = pathext
        .to_string_lossy()
        .split(';')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    let has_ext = Path::new(name).extension().is_some();

    std::env::split_paths(path)
        .filter(|dir| !dir.as_os_str().is_empty())
        .find_map(|dir| {
            let with_ext = exts.iter().map(|ext| dir.join(format!("{name}{ext}")));
            let as_given = has_ext.then(|| dir.join(name));
            with_ext
                .chain(as_given)
                .find(|candidate| is_file(candidate))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn fake_fs(files: &[&str]) -> impl Fn(&Path) -> bool {
        let files: Vec<String> = files.iter().map(|f| f.to_ascii_lowercase()).collect();
        move |p: &Path| files.contains(&p.to_string_lossy().to_ascii_lowercase())
    }

    fn path(dirs: &[&str]) -> OsString {
        std::env::join_paths(dirs).unwrap()
    }

    const PATHEXT: &str = ".COM;.EXE;.BAT;.CMD";

    #[test]
    fn finds_cmd_shim_via_pathext() {
        let fs = fake_fs(&["C:\\VSCode\\bin\\code.cmd"]);
        let found = find_on_path(
            "code",
            &path(&["C:\\Windows", "C:\\VSCode\\bin"]),
            PATHEXT.as_ref(),
            &fs,
        );
        assert_eq!(found, Some(PathBuf::from("C:\\VSCode\\bin\\code.cmd")));
    }

    #[test]
    fn earlier_directory_wins() {
        let fs = fake_fs(&["C:\\a\\tool.exe", "C:\\b\\tool.exe"]);
        let found = find_on_path("tool", &path(&["C:\\a", "C:\\b"]), PATHEXT.as_ref(), &fs);
        assert_eq!(found, Some(PathBuf::from("C:\\a\\tool.exe")));
    }

    #[test]
    fn pathext_order_wins_inside_one_directory() {
        let fs = fake_fs(&["C:\\a\\tool.exe", "C:\\a\\tool.cmd"]);
        let found = find_on_path("tool", &path(&["C:\\a"]), PATHEXT.as_ref(), &fs);
        assert_eq!(found, Some(PathBuf::from("C:\\a\\tool.exe")));
    }

    #[test]
    fn name_with_extension_is_also_tried_as_given() {
        let fs = fake_fs(&["C:\\Windows\\explorer.exe"]);
        let found = find_on_path(
            "explorer.exe",
            &path(&["C:\\Windows"]),
            PATHEXT.as_ref(),
            &fs,
        );
        assert_eq!(found, Some(PathBuf::from("C:\\Windows\\explorer.exe")));
    }

    #[test]
    fn missing_tool_is_none() {
        let fs = fake_fs(&[]);
        let found = find_on_path("code", &path(&["C:\\Windows"]), PATHEXT.as_ref(), &fs);
        assert_eq!(found, None);
    }

    #[test]
    fn empty_path_entries_are_skipped() {
        let fs = fake_fs(&["code.cmd"]);
        let found = find_on_path("code", OsStr::new(";;"), PATHEXT.as_ref(), &fs);
        assert_eq!(found, None);
    }

    #[test]
    fn scan_finds_agents_shims_and_git_bash() {
        let fs = fake_fs(&[
            r"C:\Users\me\AppData\Roaming\npm\claude.cmd",
            r"C:\Users\me\AppData\Roaming\npm\codex.cmd",
            r"C:\Program Files\Git\cmd\git.exe",
            r"C:\Program Files\Git\git-bash.exe",
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
        ]);
        let p = path(&[
            r"C:\Windows\System32\WindowsPowerShell\v1.0",
            r"C:\Program Files\Git\cmd",
            r"C:\Users\me\AppData\Roaming\npm",
        ]);
        let scan = ToolScan::scan_with(&p, PATHEXT.as_ref(), fs);
        assert_eq!(
            scan.path_of("claude"),
            Some(Path::new(r"C:\Users\me\AppData\Roaming\npm\claude.cmd"))
        );
        assert!(scan.path_of("codex").is_some());
        assert!(scan.path_of("opencode").is_none());
        assert!(scan.path_of("pwsh").is_none());
        assert!(scan.path_of("powershell").is_some());
        assert_eq!(
            scan.git_bash,
            Some(PathBuf::from(r"C:\Program Files\Git\git-bash.exe"))
        );
        assert_eq!(scan.tools.len(), TOOLS.len());
    }

    #[test]
    fn windows_terminal_alias_found_off_path() {
        let alias = r"C:\Users\me\AppData\Local\Microsoft\WindowsApps\wt.exe";
        let mut scan = ToolScan::scan_with(&path(&[r"C:\Windows"]), PATHEXT.as_ref(), fake_fs(&[]));
        assert!(scan.path_of("wt").is_none());
        scan.add_wt_fallback(
            Some(Path::new(r"C:\Users\me\AppData\Local")),
            fake_fs(&[alias]),
        );
        assert_eq!(scan.path_of("wt"), Some(Path::new(alias)));
    }

    #[test]
    fn git_bash_found_next_to_a_custom_git_install() {
        let fs = fake_fs(&[r"D:\tools\Git\git-bash.exe"]);
        let found = git_bash_near(Some(Path::new(r"D:\tools\Git\cmd\git.exe")), fs);
        assert_eq!(found, Some(PathBuf::from(r"D:\tools\Git\git-bash.exe")));
    }
}
