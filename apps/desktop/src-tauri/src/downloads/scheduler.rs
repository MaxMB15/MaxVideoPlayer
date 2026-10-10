// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Max Boksem. See NOTICE for additional terms under GPLv3 section 7.

use mvp_core::downloads::model::DownloadStatus;

/// Given the currently-active items (queued + downloading) ordered FIFO and a
/// concurrency limit, return the ids that should be started now. An item should
/// start if it is `Queued` and the number of `Downloading` items is below the
/// limit (counting items we are about to start).
pub fn ids_to_start(active: &[(String, DownloadStatus)], limit: usize) -> Vec<String> {
    let mut running = active
        .iter()
        .filter(|(_, s)| *s == DownloadStatus::Downloading)
        .count();
    let mut to_start = Vec::new();
    for (id, status) in active {
        if running >= limit {
            break;
        }
        if *status == DownloadStatus::Queued {
            to_start.push(id.clone());
            running += 1;
        }
    }
    to_start
}

#[cfg(test)]
mod tests {
    use super::*;
    use mvp_core::downloads::model::DownloadStatus::*;

    #[test]
    fn starts_up_to_limit() {
        let active = vec![
            ("a".to_string(), Downloading),
            ("b".to_string(), Queued),
            ("c".to_string(), Queued),
            ("d".to_string(), Queued),
        ];
        // limit 3, one already running → start 2 more (b, c).
        assert_eq!(ids_to_start(&active, 3), vec!["b", "c"]);
    }

    #[test]
    fn starts_nothing_when_full() {
        let active = vec![
            ("a".to_string(), Downloading),
            ("b".to_string(), Downloading),
            ("c".to_string(), Queued),
        ];
        assert!(ids_to_start(&active, 2).is_empty());
    }

    #[test]
    fn starts_all_when_under_limit() {
        let active = vec![("a".to_string(), Queued), ("b".to_string(), Queued)];
        assert_eq!(ids_to_start(&active, 5), vec!["a", "b"]);
    }
}
