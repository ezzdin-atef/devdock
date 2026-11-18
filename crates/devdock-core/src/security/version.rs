//! Loose version ordering, good enough to pick "the first fixed version" across
//! npm, Cargo and the other ecosystems Grype reports. Not a full semver parser:
//! numbers compare as numbers, anything else as text, and a pre-release
//! (`1.0.0-rc.1`) sorts before its release.

use std::cmp::Ordering;

fn parts(v: &str) -> (Vec<&str>, Option<&str>) {
    let v = v.trim().trim_start_matches(['v', 'V', '=']);
    let v = v.split('+').next().unwrap_or(v);
    let (main, pre) = match v.split_once('-') {
        Some((m, p)) => (m, Some(p)),
        None => (v, None),
    };
    (main.split('.').collect(), pre)
}

fn cmp_segment(a: &str, b: &str) -> Ordering {
    match (a.parse::<u64>(), b.parse::<u64>()) {
        (Ok(x), Ok(y)) => x.cmp(&y),
        (Ok(_), Err(_)) => Ordering::Greater,
        (Err(_), Ok(_)) => Ordering::Less,
        _ => a.cmp(b),
    }
}

pub fn compare(a: &str, b: &str) -> Ordering {
    let (am, ap) = parts(a);
    let (bm, bp) = parts(b);
    for i in 0..am.len().max(bm.len()) {
        let x = am.get(i).copied().unwrap_or("0");
        let y = bm.get(i).copied().unwrap_or("0");
        let ord = cmp_segment(x, y);
        if ord != Ordering::Equal {
            return ord;
        }
    }
    match (ap, bp) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(x), Some(y)) => {
            let xs: Vec<&str> = x.split('.').collect();
            let ys: Vec<&str> = y.split('.').collect();
            for i in 0..xs.len().max(ys.len()) {
                match (xs.get(i), ys.get(i)) {
                    (Some(p), Some(q)) => {
                        let ord = cmp_segment(p, q);
                        if ord != Ordering::Equal {
                            return ord;
                        }
                    }
                    (Some(_), None) => return Ordering::Greater,
                    (None, Some(_)) => return Ordering::Less,
                    (None, None) => {}
                }
            }
            Ordering::Equal
        }
    }
}

/// The lowest fixed version above `installed` (a scanner may list fixes for
/// several release lines, `1.2.9` and `2.0.3`). Without `installed`, the lowest.
pub fn first_fix(installed: Option<&str>, fixes: &[String]) -> Option<String> {
    let mut candidates: Vec<&String> = fixes
        .iter()
        .filter(|f| !f.trim().is_empty())
        .filter(|f| installed.is_none_or(|i| compare(f, i) == Ordering::Greater))
        .collect();
    candidates.sort_by(|a, b| compare(a, b));
    candidates.first().map(|s| (*s).clone())
}

/// The higher of two optional versions.
pub fn max(a: Option<String>, b: Option<String>) -> Option<String> {
    match (a, b) {
        (Some(x), Some(y)) => Some(if compare(&x, &y) == Ordering::Less {
            y
        } else {
            x
        }),
        (x, None) => x,
        (None, y) => y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering() {
        assert_eq!(compare("1.10.0", "1.9.9"), Ordering::Greater);
        assert_eq!(compare("1.0", "1.0.0"), Ordering::Equal);
        assert_eq!(compare("v2.0.0", "2.0.0"), Ordering::Equal);
        assert_eq!(compare("1.0.0-rc.1", "1.0.0"), Ordering::Less);
        assert_eq!(compare("1.0.0-rc.2", "1.0.0-rc.10"), Ordering::Less);
        assert_eq!(compare("1.0.0+build", "1.0.0"), Ordering::Equal);
    }

    #[test]
    fn first_fix_above_the_installed_version() {
        let fixes = vec!["2.0.3".to_owned(), "1.2.9".to_owned()];
        assert_eq!(first_fix(Some("1.2.0"), &fixes).as_deref(), Some("1.2.9"));
        assert_eq!(first_fix(Some("2.0.0"), &fixes).as_deref(), Some("2.0.3"));
        assert_eq!(first_fix(None, &fixes).as_deref(), Some("1.2.9"));
        assert_eq!(first_fix(Some("3.0.0"), &fixes), None);
    }

    #[test]
    fn max_of_options() {
        assert_eq!(
            max(Some("1.2.0".into()), Some("1.10.0".into())).as_deref(),
            Some("1.10.0")
        );
        assert_eq!(max(None, Some("1.0".into())).as_deref(), Some("1.0"));
    }
}
