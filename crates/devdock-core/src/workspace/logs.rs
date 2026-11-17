//! Combined logs (T7.5): every service's run log merged by time, each line labelled.
//! Each service keeps its own cursor, so the UI only fetches new lines.

use std::collections::HashMap;

use serde::Serialize;

use crate::run::{LogLine, LogsPage};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergedLine {
    pub service: String,
    #[serde(flatten)]
    pub line: LogLine,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergedPage {
    pub lines: Vec<MergedLine>,
    /// Service id -> next sequence number to ask for.
    pub cursors: HashMap<String, u64>,
}

/// Merges per-service pages (already filtered by each service's cursor) by
/// timestamp; equal timestamps keep each service's own order.
pub fn merge(pages: Vec<(String, LogsPage)>) -> MergedPage {
    let mut out = MergedPage::default();
    for (service, page) in pages {
        out.cursors.insert(service.clone(), page.next_seq);
        out.lines
            .extend(page.lines.into_iter().map(|line| MergedLine {
                service: service.clone(),
                line,
            }));
    }
    // Stable sort: lines from one service stay in sequence order on ties.
    out.lines.sort_by_key(|l| l.line.ts);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::Stream;

    fn page(lines: &[(u64, u64, &str)], next: u64) -> LogsPage {
        LogsPage {
            lines: lines
                .iter()
                .map(|(seq, ts, t)| LogLine {
                    seq: *seq,
                    ts: *ts,
                    stream: Stream::Out,
                    text: (*t).into(),
                })
                .collect(),
            next_seq: next,
            truncated: false,
        }
    }

    #[test]
    fn interleaves_by_time_and_labels_each_line() {
        let merged = merge(vec![
            (
                "db".into(),
                page(&[(0, 100, "db up"), (1, 300, "db ready")], 2),
            ),
            (
                "api".into(),
                page(&[(5, 200, "api boot"), (6, 300, "api listening")], 7),
            ),
        ]);
        let got: Vec<String> = merged
            .lines
            .iter()
            .map(|l| format!("{} {}", l.service, l.line.text))
            .collect();
        assert_eq!(
            got,
            vec![
                "db db up",
                "api api boot",
                "db db ready",
                "api api listening"
            ]
        );
        assert_eq!(merged.cursors["db"], 2);
        assert_eq!(merged.cursors["api"], 7);
    }
}
