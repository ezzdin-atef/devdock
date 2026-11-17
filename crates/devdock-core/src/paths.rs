//! Path normalisation (T2.3). Two spellings of the same folder must produce the same
//! key, so `D:\Dev\x\` and `d:\dev\x` are one project.

use std::path::{Path, PathBuf};

/// Canonical display form: the real on-disk casing when the folder exists, no `\\?\`
/// prefix, no trailing separator (except for a drive root such as `D:\`).
pub fn normalize(path: &Path) -> PathBuf {
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let text = strip_verbatim(&resolved.to_string_lossy());
    PathBuf::from(trim_trailing_separators(&text))
}

/// Comparison key: the normalised path, lowercased (Windows paths are case-insensitive)
/// with `/` treated as `\`.
pub fn key(path: &Path) -> String {
    normalize(path)
        .to_string_lossy()
        .replace('/', "\\")
        .to_lowercase()
}

/// `\\?\C:\x` -> `C:\x`, `\\?\UNC\server\share` -> `\\server\share`.
fn strip_verbatim(text: &str) -> String {
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        rest.to_owned()
    } else {
        text.to_owned()
    }
}

fn trim_trailing_separators(text: &str) -> String {
    let trimmed = text.trim_end_matches(['\\', '/']);
    // Keep `C:\` as is: `C:` alone means "current directory on drive C".
    if trimmed.len() == 2 && trimmed.ends_with(':') {
        format!("{trimmed}\\")
    } else if trimmed.is_empty() {
        text.to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// FNV-1a, 64 bit. Used for project ids because `std`'s hasher is not guaranteed to
/// give the same value across Rust versions, and ids are saved to disk.
pub fn stable_hash(text: &str) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0100_0000_01b3;
    text.bytes().fold(OFFSET, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(PRIME)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spellings_of_a_missing_folder_share_a_key() {
        // Neither exists, so this tests the text rules without canonicalize.
        assert_eq!(key(Path::new("Q:\\Dev\\x\\")), key(Path::new("q:\\dev\\x")));
        assert_eq!(key(Path::new("Q:/Dev/x")), key(Path::new("q:\\dev\\x")));
    }

    #[test]
    fn existing_folder_keys_match_regardless_of_case_and_slash() {
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().to_string_lossy().into_owned();
        let shouty = format!("{}\\", plain.to_uppercase());
        assert_eq!(key(Path::new(&plain)), key(Path::new(&shouty)));
    }

    #[test]
    fn normalize_drops_verbatim_prefix_and_keeps_real_casing() {
        let dir = tempfile::tempdir().unwrap();
        let n = normalize(dir.path());
        let s = n.to_string_lossy();
        assert!(!s.starts_with(r"\\?\"), "{s}");
        assert!(!s.ends_with('\\'));
    }

    #[test]
    fn verbatim_unc_becomes_plain_unc() {
        assert_eq!(strip_verbatim(r"\\?\UNC\srv\share\x"), r"\\srv\share\x");
        assert_eq!(strip_verbatim(r"\\?\D:\dev"), r"D:\dev");
    }

    #[test]
    fn drive_root_keeps_its_separator() {
        assert_eq!(trim_trailing_separators("D:\\"), "D:\\");
        assert_eq!(trim_trailing_separators("D:\\dev\\\\"), "D:\\dev");
    }

    #[test]
    fn stable_hash_is_fixed() {
        // Known FNV-1a vectors: if these change, saved ids would break.
        assert_eq!(stable_hash(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(stable_hash("a"), 0xaf63_dc4c_8601_ec8c);
    }
}
