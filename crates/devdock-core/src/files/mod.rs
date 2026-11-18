//! The Files tab: browse, read and save files inside a project folder, and search
//! across them (`search`). Every path the UI sends is relative to the project root
//! and is checked to stay inside it, so `..` or a link can't reach other files.

pub mod search;

use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::error::ErrorDto;

/// Files bigger than this open as "too large" instead of in the editor.
pub const MAX_OPEN_BYTES: u64 = 2 * 1024 * 1024;
/// A folder with more entries than this shows the first ones and a count.
pub const MAX_LIST_ENTRIES: usize = 5000;
/// How much of a file is checked for NUL bytes to call it binary.
const BINARY_SNIFF: usize = 8 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum FilesError {
    #[error("{0} is outside the project folder")]
    OutsideProject(String),
    #[error("{0} doesn't exist")]
    NotFound(String),
    #[error("{path} is {size} bytes")]
    TooLarge { path: String, size: u64 },
    #[error("{0} isn't a text file")]
    Binary(String),
    #[error("{0} changed on disk since it was opened")]
    Changed(String),
    #[error("{path}: {source}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },
}

impl From<FilesError> for ErrorDto {
    fn from(err: FilesError) -> Self {
        let (kind, fix): (&'static str, Option<&str>) = match &err {
            FilesError::OutsideProject(_) => ("outsideProject", None),
            FilesError::NotFound(_) => (
                "fileNotFound",
                Some("It may have been moved or deleted. Refresh the file list."),
            ),
            FilesError::TooLarge { .. } => (
                "fileTooLarge",
                Some("DevDock opens files up to 2 MB. Use your editor for bigger ones."),
            ),
            FilesError::Binary(_) => (
                "fileBinary",
                Some("Images, archives and other binary files open in their own programs."),
            ),
            FilesError::Changed(_) => (
                "fileChanged",
                Some("Reload to see the new version, or overwrite it with yours."),
            ),
            FilesError::Io { .. } => (
                "fileIo",
                Some("Check that the file isn't read-only or locked by another program."),
            ),
        };
        let message = match &err {
            FilesError::TooLarge { path, size } => {
                format!(
                    "{path} is {} MB, too large to open here.",
                    size / (1024 * 1024)
                )
            }
            other => {
                let text = other.to_string();
                let mut chars = text.chars();
                match chars.next() {
                    Some(c) => format!("{}{}.", c.to_uppercase(), chars.as_str()),
                    None => text,
                }
            }
        };
        ErrorDto {
            kind,
            message,
            fix: fix.map(str::to_owned),
        }
    }
}

/// One row of the file tree. `path` is relative to the project, with `/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub dir: bool,
    /// Matched by .gitignore (node_modules, build output, .env…): shown dimmed.
    pub ignored: bool,
    /// A dotfile, `.git` or Windows-hidden item (only listed with `show_hidden`).
    pub hidden: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub entries: Vec<Entry>,
    /// Entries left out past `MAX_LIST_ENTRIES`.
    pub hidden: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Eol {
    Lf,
    Crlf,
}

/// How a file is written back, so a save doesn't change what the user didn't touch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Format {
    pub eol: Eol,
    /// UTF-8 byte order mark at the start.
    pub bom: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileText {
    /// Content with `\n` line endings; `format` says how it's stored.
    pub text: String,
    pub format: Format,
    /// Last-modified time (ms since epoch) when read; sent back on save.
    pub mtime: u64,
}

/// `rel` ("src/main.rs", "" for the root) resolved inside `root`.
/// Rejects absolute paths, `..`, and links that lead outside the project.
pub fn resolve(root: &Path, rel: &str) -> Result<PathBuf, FilesError> {
    let rel_path = Path::new(rel);
    let mut joined = root.to_path_buf();
    for part in rel_path.components() {
        match part {
            Component::Normal(p) => joined.push(p),
            Component::CurDir => {}
            _ => return Err(FilesError::OutsideProject(rel.to_owned())),
        }
    }
    let real_root = fs::canonicalize(root).map_err(|e| io_err(rel, e))?;
    let real = fs::canonicalize(&joined).map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            FilesError::NotFound(rel.to_owned())
        } else {
            io_err(rel, e)
        }
    })?;
    if !real.starts_with(&real_root) {
        return Err(FilesError::OutsideProject(rel.to_owned()));
    }
    Ok(joined)
}

fn io_err(path: &str, source: io::Error) -> FilesError {
    FilesError::Io {
        path: path.to_owned(),
        source,
    }
}

fn join_rel(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_owned()
    } else {
        format!("{}/{name}", dir.trim_end_matches('/'))
    }
}

/// Hidden the way file managers mean it: a name starting with `.`, or (on
/// Windows) the Hidden attribute.
pub fn is_hidden(name: &std::ffi::OsStr, meta: Option<&fs::Metadata>) -> bool {
    if name.to_string_lossy().starts_with('.') {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        if meta.is_some_and(|m| m.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0) {
            return true;
        }
    }
    #[cfg(not(windows))]
    let _ = meta;
    false
}

/// The folder `dir` (relative; "" = root): folders first, then files, by name.
/// Hidden entries (dotfiles, `.git`, the Windows Hidden attribute) only with
/// `show_hidden`; gitignored entries are kept but marked.
pub fn list(root: &Path, dir: &str, show_hidden: bool) -> Result<Listing, FilesError> {
    let abs = resolve(root, dir)?;
    let read = fs::read_dir(&abs).map_err(|e| io_err(dir, e))?;

    // What a gitignore-aware walk keeps at depth 1; anything else is ignored.
    let kept: HashSet<std::ffi::OsString> = ignore::WalkBuilder::new(&abs)
        .max_depth(Some(1))
        .hidden(false)
        .parents(true)
        .require_git(false)
        .build()
        .filter_map(Result::ok)
        .filter(|e| e.depth() == 1)
        .map(|e| e.file_name().to_owned())
        .collect();

    let mut entries: Vec<Entry> = read
        .filter_map(Result::ok)
        .map(|e| {
            let hidden = is_hidden(&e.file_name(), e.metadata().ok().as_ref());
            (e, hidden)
        })
        .filter(|(_, hidden)| show_hidden || !hidden)
        .map(|(e, hidden)| {
            let name = e.file_name().to_string_lossy().into_owned();
            // Follows links, so a linked folder still opens like a folder.
            let is_dir = e.path().is_dir();
            Entry {
                path: join_rel(dir, &name),
                ignored: !kept.contains(&e.file_name()),
                hidden,
                dir: is_dir,
                name,
            }
        })
        .collect();
    entries.sort_by(|a, b| {
        b.dir
            .cmp(&a.dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    let hidden = entries.len().saturating_sub(MAX_LIST_ENTRIES);
    entries.truncate(MAX_LIST_ENTRIES);
    Ok(Listing { entries, hidden })
}

fn mtime_ms(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Last-modified time of `path`, or `None` if it's gone.
pub fn stat(root: &Path, path: &str) -> Result<Option<u64>, FilesError> {
    match resolve(root, path) {
        Ok(abs) => Ok(fs::metadata(abs).ok().map(|m| mtime_ms(&m))),
        Err(FilesError::NotFound(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Reads a text file for the editor.
pub fn read(root: &Path, path: &str) -> Result<FileText, FilesError> {
    let abs = resolve(root, path)?;
    let meta = fs::metadata(&abs).map_err(|e| io_err(path, e))?;
    if meta.is_dir() {
        return Err(FilesError::Binary(path.to_owned()));
    }
    if meta.len() > MAX_OPEN_BYTES {
        return Err(FilesError::TooLarge {
            path: path.to_owned(),
            size: meta.len(),
        });
    }
    let bytes = fs::read(&abs).map_err(|e| io_err(path, e))?;
    let (text, format) = decode(&bytes).ok_or_else(|| FilesError::Binary(path.to_owned()))?;
    Ok(FileText {
        text,
        format,
        mtime: mtime_ms(&meta),
    })
}

/// Bytes to editor text: `None` when they don't look like UTF-8 text.
/// Line endings become `\n`; the more common style is remembered for saving.
pub fn decode(bytes: &[u8]) -> Option<(String, Format)> {
    let sniff = &bytes[..bytes.len().min(BINARY_SNIFF)];
    if sniff.contains(&0) {
        return None;
    }
    let (bom, body) = match bytes.strip_prefix(b"\xEF\xBB\xBF") {
        Some(rest) => (true, rest),
        None => (false, bytes),
    };
    let text = std::str::from_utf8(body).ok()?;
    let crlf = text.matches("\r\n").count();
    let lf = text.matches('\n').count() - crlf;
    let eol = if crlf > lf { Eol::Crlf } else { Eol::Lf };
    let text = text.replace("\r\n", "\n");
    Some((text, Format { eol, bom }))
}

/// Editor text back to bytes in the file's own format.
pub fn encode(text: &str, format: Format) -> Vec<u8> {
    let normalized = text.replace("\r\n", "\n");
    let body = match format.eol {
        Eol::Lf => normalized,
        Eol::Crlf => normalized.replace('\n', "\r\n"),
    };
    let mut out = Vec::with_capacity(body.len() + 3);
    if format.bom {
        out.extend_from_slice(b"\xEF\xBB\xBF");
    }
    out.extend_from_slice(body.as_bytes());
    out
}

/// Saves `text` over an existing file. Unless `force`, refuses when the file
/// changed on disk since `expected_mtime`. Writes a temp file next to it and
/// renames it over, so a crash never leaves half a file. Returns the new mtime.
pub fn write(
    root: &Path,
    path: &str,
    text: &str,
    format: Format,
    expected_mtime: u64,
    force: bool,
) -> Result<u64, FilesError> {
    let abs = resolve(root, path)?;
    let meta = fs::metadata(&abs).map_err(|e| io_err(path, e))?;
    if !force && mtime_ms(&meta) != expected_mtime {
        return Err(FilesError::Changed(path.to_owned()));
    }
    if meta.permissions().readonly() {
        return Err(io_err(
            path,
            io::Error::new(io::ErrorKind::PermissionDenied, "the file is read-only"),
        ));
    }
    let dir = abs
        .parent()
        .ok_or_else(|| FilesError::OutsideProject(path.to_owned()))?;
    let name = abs
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.devdock-save"));
    let bytes = encode(text, format);
    let written = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, &abs)
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(io_err(path, e));
    }
    let meta = fs::metadata(&abs).map_err(|e| io_err(path, e))?;
    Ok(mtime_ms(&meta))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::create_dir_all(dir.path().join("node_modules/x")).unwrap();
        fs::create_dir_all(dir.path().join(".git")).unwrap();
        fs::write(dir.path().join(".gitignore"), "node_modules\n").unwrap();
        fs::write(dir.path().join("src/main.rs"), "fn main() {}\r\n").unwrap();
        fs::write(dir.path().join("README.md"), "# hi\n").unwrap();
        dir
    }

    #[test]
    fn rejects_paths_outside_the_project() {
        let dir = project();
        assert!(matches!(
            resolve(dir.path(), "../x"),
            Err(FilesError::OutsideProject(_))
        ));
        assert!(matches!(
            resolve(dir.path(), "src/../../x"),
            Err(FilesError::OutsideProject(_))
        ));
        let abs = dir.path().join("README.md");
        assert!(matches!(
            resolve(dir.path(), &abs.to_string_lossy()),
            Err(FilesError::OutsideProject(_))
        ));
        assert!(resolve(dir.path(), "src/main.rs").is_ok());
    }

    #[test]
    fn lists_folders_first_hides_dotfiles_and_marks_ignored() {
        let dir = project();
        let listing = list(dir.path(), "", false).unwrap();
        let names: Vec<_> = listing.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["node_modules", "src", "README.md"]);
        assert!(listing.entries[0].ignored);
        assert!(!listing.entries[1].ignored);
        let inner = list(dir.path(), "src", false).unwrap();
        assert_eq!(inner.entries[0].path, "src/main.rs");
    }

    #[test]
    fn show_hidden_lists_dotfiles_and_git() {
        let dir = project();
        let listing = list(dir.path(), "", true).unwrap();
        let names: Vec<_> = listing.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            [".git", "node_modules", "src", ".gitignore", "README.md"]
        );
        let hidden: Vec<_> = listing.entries.iter().map(|e| e.hidden).collect();
        assert_eq!(hidden, [true, false, false, true, false]);
    }

    #[cfg(windows)]
    #[test]
    fn windows_hidden_attribute_counts_as_hidden() {
        let dir = project();
        let path = dir.path().join("secret.txt");
        fs::write(&path, "x").unwrap();
        let status = std::process::Command::new("attrib")
            .arg("+h")
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success());
        let shown = |hidden| -> Vec<String> {
            list(dir.path(), "", hidden)
                .unwrap()
                .entries
                .into_iter()
                .map(|e| e.name)
                .collect()
        };
        assert!(!shown(false).contains(&"secret.txt".to_owned()));
        assert!(shown(true).contains(&"secret.txt".to_owned()));
    }

    #[test]
    fn keeps_crlf_and_bom_through_a_save() {
        let dir = project();
        fs::write(dir.path().join("w.txt"), b"\xEF\xBB\xBFa\r\nb\r\n").unwrap();
        let file = read(dir.path(), "w.txt").unwrap();
        assert_eq!(file.text, "a\nb\n");
        assert_eq!(
            file.format,
            Format {
                eol: Eol::Crlf,
                bom: true
            }
        );
        write(
            dir.path(),
            "w.txt",
            "a\nb\nc\n",
            file.format,
            file.mtime,
            false,
        )
        .unwrap();
        assert_eq!(
            fs::read(dir.path().join("w.txt")).unwrap(),
            b"\xEF\xBB\xBFa\r\nb\r\nc\r\n"
        );
        assert!(!dir.path().join(".w.txt.devdock-save").exists());
    }

    #[test]
    fn refuses_to_overwrite_a_file_changed_on_disk() {
        let dir = project();
        let file = read(dir.path(), "README.md").unwrap();
        let err = write(
            dir.path(),
            "README.md",
            "x",
            file.format,
            file.mtime + 1,
            false,
        );
        assert!(matches!(err, Err(FilesError::Changed(_))));
        write(
            dir.path(),
            "README.md",
            "x",
            file.format,
            file.mtime + 1,
            true,
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("README.md")).unwrap(),
            "x"
        );
    }

    #[test]
    fn binary_files_dont_open() {
        let dir = project();
        fs::write(dir.path().join("a.bin"), [1u8, 0, 2]).unwrap();
        assert!(matches!(
            read(dir.path(), "a.bin"),
            Err(FilesError::Binary(_))
        ));
    }
}
