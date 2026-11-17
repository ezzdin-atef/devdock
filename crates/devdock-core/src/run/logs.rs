//! Per-run log ring buffer (T4.5). Lines get increasing sequence numbers so the UI
//! can ask "everything after N" and notice when old lines were dropped.

use std::collections::VecDeque;

use serde::Serialize;

pub const DEFAULT_CAPACITY: usize = 5_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Stream {
    Out,
    Err,
    /// Lines DevDock writes itself ("▶ npm run dev", "■ exited with code 1").
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub seq: u64,
    pub ts: u64,
    pub stream: Stream,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsPage {
    pub lines: Vec<LogLine>,
    /// Pass this as `since` next time.
    pub next_seq: u64,
    /// Lines between the requested `since` and the oldest kept line were dropped.
    pub truncated: bool,
}

#[derive(Debug)]
pub struct LogBuffer {
    lines: VecDeque<LogLine>,
    capacity: usize,
    next_seq: u64,
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

impl LogBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            lines: VecDeque::with_capacity(capacity.min(1024)),
            capacity: capacity.max(1),
            next_seq: 0,
        }
    }

    pub fn push(&mut self, stream: Stream, text: String, ts: u64) -> u64 {
        if self.lines.len() == self.capacity {
            self.lines.pop_front();
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        self.lines.push_back(LogLine {
            seq,
            ts,
            stream,
            text,
        });
        seq
    }

    /// Lines with `seq >= since`, oldest first.
    pub fn since(&self, since: u64) -> LogsPage {
        let first = self.lines.front().map_or(self.next_seq, |l| l.seq);
        LogsPage {
            lines: self
                .lines
                .iter()
                .skip_while(|l| l.seq < since)
                .cloned()
                .collect(),
            next_seq: self.next_seq,
            truncated: since < first,
        }
    }

    /// Drops every line. Sequence numbers keep counting, so a viewer that asks for
    /// lines after its last one simply gets the new ones.
    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// The last line on stderr, else the last process line: shown next to FAILED.
    pub fn last_error_line(&self) -> Option<&str> {
        let rev = || {
            self.lines
                .iter()
                .rev()
                .filter(|l| !l.text.trim().is_empty())
        };
        rev()
            .find(|l| l.stream == Stream::Err)
            .or_else(|| rev().find(|l| l.stream == Stream::Out))
            .map(|l| l.text.as_str())
    }
}

/// Cleans one raw line from a process: drops ANSI colour/cursor codes and keeps only
/// the last `\r`-separated segment (progress bars redraw a line with `\r`).
pub fn clean_line(raw: &str) -> String {
    let raw = raw.trim_end_matches(['\n', '\r']);
    let segment = raw
        .rsplit('\r')
        .find(|s| !strip_ansi(s).trim().is_empty())
        .unwrap_or("");
    strip_ansi(segment)
}

pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            // CSI: ESC [ params final-byte(@..~)
            Some('[') => {
                chars.next();
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            // OSC: ESC ] ... BEL or ESC \
            Some(']') => {
                chars.next();
                while let Some(c) = chars.next() {
                    if c == '\u{7}' || (c == '\u{1b}' && chars.peek() == Some(&'\\')) {
                        if c == '\u{1b}' {
                            chars.next();
                        }
                        break;
                    }
                }
            }
            Some(_) => {
                chars.next();
            }
            None => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_drops_oldest_and_reports_truncation() {
        let mut b = LogBuffer::new(3);
        for i in 0..5 {
            b.push(Stream::Out, format!("l{i}"), 0);
        }
        let page = b.since(0);
        assert!(page.truncated);
        assert_eq!(
            page.lines
                .iter()
                .map(|l| l.text.as_str())
                .collect::<Vec<_>>(),
            vec!["l2", "l3", "l4"]
        );
        assert_eq!(page.next_seq, 5);
        let tail = b.since(4);
        assert!(!tail.truncated);
        assert_eq!(tail.lines.len(), 1);
        assert!(b.since(5).lines.is_empty());
    }

    #[test]
    fn last_error_prefers_stderr() {
        let mut b = LogBuffer::new(10);
        b.push(Stream::Err, "Error: port in use".into(), 0);
        b.push(Stream::Out, "bye".into(), 0);
        b.push(Stream::System, "■ exited".into(), 0);
        assert_eq!(b.last_error_line(), Some("Error: port in use"));
        let mut only_out = LogBuffer::new(10);
        only_out.push(Stream::Out, "last words".into(), 0);
        assert_eq!(only_out.last_error_line(), Some("last words"));
    }

    #[test]
    fn ansi_codes_are_removed() {
        assert_eq!(
            strip_ansi("\u{1b}[32m✓\u{1b}[39m Ready in \u{1b}[1m1.2s\u{1b}[22m"),
            "✓ Ready in 1.2s"
        );
        assert_eq!(strip_ansi("\u{1b}]0;title\u{7}hello"), "hello");
        assert_eq!(strip_ansi("\u{1b}[2K\u{1b}[1Gdone"), "done");
    }

    #[test]
    fn carriage_returns_keep_the_final_redraw() {
        assert_eq!(clean_line("10%\r50%\r100%\r\n"), "100%");
        assert_eq!(clean_line("ok\r\n"), "ok");
        assert_eq!(clean_line("\u{1b}[2K\r"), "");
    }
}
