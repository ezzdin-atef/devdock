//! The DevDock block in the Windows hosts file, for `*.test` names (browsers
//! resolve `*.localhost` on their own, `*.test` needs an entry per name).
//!
//! Only lines between the two markers are DevDock's; everything else is kept as is.
//! Writing the file needs administrator rights, so the Tauri layer does that through
//! an elevated copy (UAC prompt) of the text built here.

use std::path::PathBuf;

pub const BEGIN: &str = "# >>> DevDock (managed, edit in DevDock)";
pub const END: &str = "# <<< DevDock";

pub fn hosts_path() -> PathBuf {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    PathBuf::from(root).join("System32\\drivers\\etc\\hosts")
}

/// The names currently in DevDock's block.
pub fn managed(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let t = line.trim();
        if t == BEGIN {
            inside = true;
        } else if t == END {
            inside = false;
        } else if inside && let Some(name) = t.split_whitespace().nth(1) {
            let name = name.to_ascii_lowercase();
            if !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

/// Every name the file maps, inside DevDock's block or not (comments skipped).
pub fn listed(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.split('#').next().unwrap_or(""))
        .flat_map(|l| l.split_whitespace().skip(1))
        .map(str::to_ascii_lowercase)
        .collect()
}

/// `text` with DevDock's block replaced by one for `names` (removed if empty).
/// Keeps the file's line endings (CRLF unless it only uses LF).
pub fn merge(text: &str, names: &[String]) -> String {
    let nl = if text.contains("\r\n") || !text.contains('\n') {
        "\r\n"
    } else {
        "\n"
    };
    let mut kept: Vec<&str> = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        match line.trim() {
            t if t == BEGIN => inside = true,
            t if t == END => inside = false,
            _ if inside => {}
            _ => kept.push(line),
        }
    }
    while kept.last().is_some_and(|l| l.trim().is_empty()) {
        kept.pop();
    }
    let mut out = kept.join(nl);
    if !names.is_empty() {
        if !out.is_empty() {
            out.push_str(nl);
            out.push_str(nl);
        }
        out.push_str(BEGIN);
        for n in names {
            out.push_str(nl);
            out.push_str(&format!("127.0.0.1 {n}{nl}::1 {n}"));
        }
        out.push_str(nl);
        out.push_str(END);
    }
    out.push_str(nl);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_replaces_and_removes_the_block() {
        let original = "# comment\r\n127.0.0.1 mine\r\n";
        let names = vec!["shop.test".to_owned()];
        let with = merge(original, &names);
        assert!(with.starts_with("# comment\r\n127.0.0.1 mine\r\n\r\n# >>> DevDock"));
        assert!(with.contains("127.0.0.1 shop.test\r\n::1 shop.test\r\n# <<< DevDock\r\n"));
        assert_eq!(managed(&with), names);
        assert!(listed(&with).contains(&"mine".to_owned()));
        assert!(listed(&with).contains(&"shop.test".to_owned()));

        let names2 = vec!["api.test".to_owned(), "shop.test".to_owned()];
        let again = merge(&with, &names2);
        assert_eq!(managed(&again), names2);
        assert_eq!(again.matches(BEGIN).count(), 1);

        assert_eq!(merge(&again, &[]), original);
    }

    #[test]
    fn lf_files_stay_lf_and_empty_files_work() {
        let out = merge("127.0.0.1 a\n", &["x.test".to_owned()]);
        assert!(!out.contains('\r'));
        assert!(merge("", &["x.test".to_owned()]).starts_with(BEGIN));
        assert_eq!(merge("", &[]), "\r\n");
    }
}
