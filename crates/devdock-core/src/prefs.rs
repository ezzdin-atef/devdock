//! User preferences (T6.5) and keyboard shortcuts (T6.4). Saved in `devdock.json`.

use serde::{Deserialize, Serialize};

use crate::agents::{AgentId, TerminalKind};
use crate::project::ProjectId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Command used by "Open in editor": `code`, `cursor`, ...
    pub editor: String,
    pub terminal: TerminalKind,
    pub agent: AgentId,
    /// Check a command's known port before starting it (T5.5).
    pub check_ports: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            editor: "code".into(),
            terminal: TerminalKind::WindowsTerminal,
            agent: AgentId::Claude,
            check_ports: true,
        }
    }
}

/// Editors offered in Settings, as (command, name). Any of them must be on PATH.
pub const EDITORS: [(&str, &str); 5] = [
    ("code", "VS Code"),
    ("cursor", "Cursor"),
    ("code-insiders", "VS Code Insiders"),
    ("windsurf", "Windsurf"),
    ("zed", "Zed"),
];

pub fn valid_editor(cmd: &str) -> bool {
    EDITORS.iter().any(|(c, _)| *c == cmd)
}

/// `Ctrl Alt <slot>` launches this, while DevDock has focus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shortcut {
    /// 1 to 9.
    pub slot: u8,
    pub project_id: ProjectId,
    /// `None` = just open the terminal.
    pub agent: Option<AgentId>,
    pub terminal: TerminalKind,
    pub subdir: Option<String>,
}

pub const SLOTS: std::ops::RangeInclusive<u8> = 1..=9;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_settings_fields_take_defaults() {
        let s: Settings = serde_json::from_str(r#"{"editor":"cursor"}"#).unwrap();
        assert_eq!(s.editor, "cursor");
        assert_eq!(s.terminal, TerminalKind::WindowsTerminal);
        assert_eq!(s.agent, AgentId::Claude);
    }

    #[test]
    fn editor_list_is_closed() {
        assert!(valid_editor("code"));
        assert!(!valid_editor("rm -rf"));
    }
}
