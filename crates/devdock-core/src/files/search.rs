//! Search across a project's files with ripgrep's own libraries: .gitignore is
//! respected, binary files are skipped, and a search can be cancelled.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use grep_matcher::Matcher;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::sinks::Lossy;
use grep_searcher::{BinaryDetection, SearcherBuilder};
use ignore::overrides::OverrideBuilder;
use serde::{Deserialize, Serialize};

use crate::error::ErrorDto;

/// The search stops after this many matches and says so.
pub const MAX_MATCHES: usize = 2000;
/// Files bigger than this are skipped (minified bundles, dumps).
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
/// Lines longer than this are cut around the first match.
const MAX_LINE_CHARS: usize = 240;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub text: String,
    #[serde(default)]
    pub regex: bool,
    #[serde(default)]
    pub match_case: bool,
    #[serde(default)]
    pub whole_word: bool,
    /// Comma-separated globs such as `src/**, *.ts`; empty = everything.
    #[serde(default)]
    pub include: String,
    /// Comma-separated globs to leave out.
    #[serde(default)]
    pub exclude: String,
    /// Also search dotfiles and Windows-hidden files (never `.git`).
    #[serde(default)]
    pub hidden: bool,
}

/// One matching line. Columns are UTF-16 offsets into `text` (what the editor uses).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineMatch {
    /// 1-based.
    pub line: u64,
    /// The line as shown (possibly cut; `offset` UTF-16 units were dropped in front).
    pub text: String,
    pub offset: usize,
    /// `[start, end)` in `text`, UTF-16 units.
    pub ranges: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMatches {
    pub path: String,
    pub matches: Vec<LineMatch>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub files: Vec<FileMatches>,
    pub total: usize,
    /// Stopped at `MAX_MATCHES`.
    pub truncated: bool,
    /// A newer search took over.
    pub cancelled: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("{0}")]
    BadPattern(String),
    #[error("{0}")]
    BadGlob(String),
}

impl From<SearchError> for ErrorDto {
    fn from(err: SearchError) -> Self {
        match err {
            SearchError::BadPattern(detail) => ErrorDto {
                kind: "badPattern",
                message: format!("That isn't a valid regular expression: {detail}"),
                fix: Some(
                    "Fix the pattern, or turn off .* to search for the text as typed.".into(),
                ),
            },
            SearchError::BadGlob(detail) => ErrorDto {
                kind: "badGlob",
                message: format!("A file filter isn't a valid pattern: {detail}"),
                fix: Some("Use patterns such as src/**, *.ts or !*.test.ts.".into()),
            },
        }
    }
}

fn globs(text: &str) -> impl Iterator<Item = &str> {
    text.split(',').map(str::trim).filter(|g| !g.is_empty())
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Cuts a long line to a window around its first match. Byte ranges in, the
/// shown text and UTF-16 ranges out.
fn shape(line: &str, byte_ranges: &[(usize, usize)]) -> (String, usize, Vec<(usize, usize)>) {
    let line = line.trim_end_matches(['\r', '\n']);
    let chars = line.chars().count();
    let (start_byte, end_byte) = if chars <= MAX_LINE_CHARS {
        (0, line.len())
    } else {
        let first = byte_ranges.first().map_or(0, |r| r.0).min(line.len());
        // Up to 40 characters of context before the match.
        let lead = line[..first]
            .char_indices()
            .rev()
            .nth(39)
            .map_or(0, |(i, _)| i);
        let end = line[lead..]
            .char_indices()
            .nth(MAX_LINE_CHARS)
            .map_or(line.len(), |(i, _)| lead + i);
        (lead, end)
    };
    let shown = &line[start_byte..end_byte];
    let offset = utf16_len(&line[..start_byte]);
    let ranges = byte_ranges
        .iter()
        .filter(|(s, e)| *s >= start_byte && *e <= end_byte && s < e)
        .filter(|(s, e)| line.is_char_boundary(*s) && line.is_char_boundary(*e))
        .map(|(s, e)| {
            let a = utf16_len(&line[start_byte..*s]);
            (a, a + utf16_len(&line[*s..*e]))
        })
        .collect();
    (shown.to_owned(), offset, ranges)
}

/// Searches every non-ignored text file under `root`. Checks `cancel` between
/// files; a cancelled search returns what it found with `cancelled` set.
pub fn search(
    root: &Path,
    query: &SearchQuery,
    cancel: &AtomicBool,
) -> Result<SearchResult, SearchError> {
    let mut result = SearchResult::default();
    if query.text.is_empty() {
        return Ok(result);
    }
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(!query.match_case)
        .word(query.whole_word)
        .fixed_strings(!query.regex)
        .line_terminator(Some(b'\n'))
        .build(&query.text)
        .map_err(|e| SearchError::BadPattern(e.to_string()))?;

    let mut overrides = OverrideBuilder::new(root);
    for g in globs(&query.include) {
        overrides
            .add(g)
            .map_err(|e| SearchError::BadGlob(e.to_string()))?;
    }
    for g in globs(&query.exclude) {
        let negated = format!("!{}", g.trim_start_matches('!'));
        overrides
            .add(&negated)
            .map_err(|e| SearchError::BadGlob(e.to_string()))?;
    }
    let overrides = overrides
        .build()
        .map_err(|e| SearchError::BadGlob(e.to_string()))?;

    let walker = ignore::WalkBuilder::new(root)
        // The walker's own `hidden` check covers dotfiles and, on Windows, the
        // Hidden attribute.
        .hidden(!query.hidden)
        .require_git(false)
        .overrides(overrides)
        .max_filesize(Some(MAX_FILE_BYTES))
        .filter_entry(|e| e.file_name() != ".git")
        .sort_by_file_path(|a, b| a.cmp(b))
        .build();

    let mut searcher = SearcherBuilder::new()
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .line_number(true)
        .build();

    for entry in walker.filter_map(Result::ok) {
        if cancel.load(Ordering::Relaxed) {
            result.cancelled = true;
            break;
        }
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/");
        let mut matches = Vec::new();
        let budget = MAX_MATCHES - result.total;
        let sink = Lossy(|line_number, line: &str| {
            let mut ranges = Vec::new();
            matcher.find_iter(line.as_bytes(), |m| {
                ranges.push((m.start(), m.end()));
                true
            })?;
            let (text, offset, ranges) = shape(line, &ranges);
            matches.push(LineMatch {
                line: line_number,
                text,
                offset,
                ranges,
            });
            Ok(matches.len() < budget)
        });
        // Unreadable files are skipped, as ripgrep does.
        if searcher.search_path(&matcher, entry.path(), sink).is_err() {
            continue;
        }
        if matches.is_empty() {
            continue;
        }
        result.total += matches.len();
        result.files.push(FileMatches { path: rel, matches });
        if result.total >= MAX_MATCHES {
            result.truncated = true;
            break;
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::create_dir_all(dir.path().join("node_modules/x")).unwrap();
        fs::write(dir.path().join(".gitignore"), "node_modules\n").unwrap();
        fs::write(dir.path().join("src/a.ts"), "const Foo = 1;\nfoo();\n").unwrap();
        fs::write(dir.path().join("src/b.rs"), "fn foobar() {}\n").unwrap();
        fs::write(dir.path().join("node_modules/x/i.js"), "foo\n").unwrap();
        fs::write(dir.path().join("bin.dat"), b"foo\x00\x01").unwrap();
        dir
    }

    fn run(root: &Path, q: SearchQuery) -> SearchResult {
        search(root, &q, &AtomicBool::new(false)).unwrap()
    }

    fn q(text: &str) -> SearchQuery {
        SearchQuery {
            text: text.into(),
            ..Default::default()
        }
    }

    #[test]
    fn finds_text_and_skips_ignored_and_binary_files() {
        let dir = project();
        let r = run(dir.path(), q("foo"));
        let paths: Vec<_> = r.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["src/a.ts", "src/b.rs"]);
        assert_eq!(r.total, 3);
        let first = &r.files[0].matches[0];
        assert_eq!(first.line, 1);
        assert_eq!(first.ranges, [(6, 9)]);
    }

    #[test]
    fn honours_case_word_regex_and_globs() {
        let dir = project();
        let mut query = q("Foo");
        query.match_case = true;
        assert_eq!(run(dir.path(), query).total, 1);

        let mut query = q("foo");
        query.whole_word = true;
        assert_eq!(run(dir.path(), query).total, 2);

        let mut query = q("foo\\w+");
        query.regex = true;
        assert_eq!(run(dir.path(), query).files[0].path, "src/b.rs");

        let mut query = q("foo");
        query.include = "*.rs".into();
        assert_eq!(run(dir.path(), query).total, 1);

        let mut query = q("foo");
        query.exclude = "*.rs, *.md".into();
        assert_eq!(run(dir.path(), query).total, 2);
    }

    #[test]
    fn hidden_files_are_searched_only_when_asked() {
        let dir = project();
        fs::write(dir.path().join(".env"), "foo=1\n").unwrap();
        fs::create_dir_all(dir.path().join(".git")).unwrap();
        fs::write(dir.path().join(".git/config"), "foo\n").unwrap();
        let paths = |hidden| -> Vec<String> {
            let mut query = q("foo");
            query.hidden = hidden;
            run(dir.path(), query)
                .files
                .into_iter()
                .map(|f| f.path)
                .collect()
        };
        assert!(!paths(false).contains(&".env".to_owned()));
        let with_hidden = paths(true);
        assert!(with_hidden.contains(&".env".to_owned()));
        assert!(!with_hidden.iter().any(|p| p.starts_with(".git/")));
    }

    #[test]
    fn plain_text_mode_treats_symbols_literally() {
        let dir = project();
        assert_eq!(run(dir.path(), q("foo();")).total, 1);
        let mut bad = q("(");
        bad.regex = true;
        assert!(search(dir.path(), &bad, &AtomicBool::new(false)).is_err());
    }

    #[test]
    fn long_lines_are_cut_around_the_match_in_utf16_units() {
        let line = format!("{}é needle", "x".repeat(400));
        let start = line.find("needle").unwrap();
        let (text, offset, ranges) = shape(&line, &[(start, start + 6)]);
        assert!(text.chars().count() <= MAX_LINE_CHARS);
        let (a, b) = ranges[0];
        let units: Vec<u16> = text.encode_utf16().collect();
        assert_eq!(String::from_utf16(&units[a..b]).unwrap(), "needle");
        assert_eq!(offset + a, utf16_len(&line[..start]));
    }

    #[test]
    fn a_cancelled_search_says_so() {
        let dir = project();
        let r = search(dir.path(), &q("foo"), &AtomicBool::new(true)).unwrap();
        assert!(r.cancelled);
        assert!(r.files.is_empty());
    }
}
