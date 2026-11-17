//! Groups: folders the user makes to organise the project list ("Client work",
//! "Experiments"). A project is in at most one group, like a file in a folder.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GroupId(pub String);

/// Named colours a new group cycles through. Any `#rrggbb` is allowed too.
pub const COLORS: [&str; 18] = [
    "amber", "blue", "green", "violet", "rose", "teal", "orange", "slate", "red", "yellow", "lime",
    "emerald", "cyan", "sky", "indigo", "purple", "pink", "stone",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: GroupId,
    pub name: String,
    #[serde(default = "default_color")]
    pub color: String,
}

fn default_color() -> String {
    COLORS[0].to_owned()
}

/// A named colour, or a custom one as `#rrggbb`.
pub fn valid_color(c: &str) -> bool {
    COLORS.contains(&c)
        || (c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|ch| ch.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_and_custom_colours() {
        assert!(valid_color("teal"));
        assert!(valid_color("#1a2B3c"));
        assert!(!valid_color("#12345"));
        assert!(!valid_color("#12345g"));
        assert!(!valid_color("red; background: url(x)"));
    }
}
