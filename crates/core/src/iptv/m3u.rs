use crate::models::channel::Channel;
use rayon::prelude::*;
use std::io::BufRead;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum M3uError {
    #[error("invalid M3U format: missing #EXTM3U header")]
    MissingHeader,
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error at line {line}: {message}")]
    Parse { line: usize, message: String },
}

/// Parse an M3U playlist from an in-memory string.
/// Good for small/medium playlists. For large files use `parse_m3u_file`.
pub fn parse_m3u(content: &str) -> Result<Vec<Channel>, M3uError> {
    let trimmed = strip_bom(content.trim());
    if !trimmed.starts_with("#EXTM3U") {
        return Err(M3uError::MissingHeader);
    }

    let lines: Vec<&str> = trimmed.lines().collect();
    let blocks = collect_blocks_from_lines(&lines);

    let channels: Vec<Channel> = blocks
        .par_iter()
        .enumerate()
        .filter_map(|(idx, (extinf, url))| parse_extinf_block(idx, extinf, url))
        .collect();

    Ok(channels)
}

/// Parse an M3U file from disk using a memory-mapped view of the file.
/// The OS maps the file into virtual memory without copying it — no per-line
/// allocations, and parallel block detection runs across all CPU cores.
pub fn parse_m3u_file(path: &Path) -> Result<Vec<Channel>, M3uError> {
    let file = std::fs::File::open(path)?;
    // SAFETY: the file is read-only and not modified while the map is live.
    let mmap = unsafe { memmap2::Mmap::map(&file)? };
    parse_m3u_bytes(&mmap)
}

/// Parse M3U from a byte slice (e.g. a memory-mapped file).
/// Lines are zero-copy &str slices into the input; blocks are found in parallel
/// via par_windows(2) and parsed in parallel with rayon.
fn parse_m3u_bytes(bytes: &[u8]) -> Result<Vec<Channel>, M3uError> {
    let content = std::str::from_utf8(bytes).map_err(|e| M3uError::Parse {
        line: 0,
        message: format!("UTF-8 decode error: {e}"),
    })?;
    let content = strip_bom(content.trim_start());
    if !content.starts_with("#EXTM3U") {
        return Err(M3uError::MissingHeader);
    }

    // Collect lines as &str slices into the mmap — no heap allocation per line.
    let lines: Vec<&str> = content.lines().collect();

    // Find (extinf, url) pairs in parallel. par_windows(2) examines every
    // consecutive pair of lines across all rayon threads simultaneously.
    let blocks: Vec<(&str, &str)> = lines
        .par_windows(2)
        .filter_map(|pair| {
            let a = pair[0].trim();
            let b = pair[1].trim();
            if a.starts_with("#EXTINF:") && !b.is_empty() && !b.starts_with('#') {
                Some((a, b))
            } else {
                None
            }
        })
        .collect();

    let channels: Vec<Channel> = blocks
        .par_iter()
        .enumerate()
        .filter_map(|(idx, (extinf, url))| parse_extinf_block(idx, extinf, url))
        .collect();

    Ok(channels)
}

/// Stream-parse M3U from any `BufRead` source. Collects EXTINF+URL pairs
/// line-by-line, then parses them in parallel with rayon.
pub fn parse_m3u_reader<R: BufRead>(reader: R) -> Result<Vec<Channel>, M3uError> {
    let mut lines_iter = reader.lines();

    let first_line = lines_iter
        .next()
        .ok_or(M3uError::MissingHeader)?
        .map_err(|e| M3uError::Parse {
            line: 0,
            message: e.to_string(),
        })?;
    let first_trimmed = strip_bom(first_line.trim());
    if !first_trimmed.starts_with("#EXTM3U") {
        return Err(M3uError::MissingHeader);
    }

    let mut blocks: Vec<(String, String)> = Vec::new();
    let mut current_extinf: Option<String> = None;

    for line_result in lines_iter {
        let line = line_result.map_err(|e| M3uError::Parse {
            line: blocks.len(),
            message: e.to_string(),
        })?;
        let trimmed = line.trim();

        if trimmed.starts_with("#EXTINF:") {
            current_extinf = Some(trimmed.to_string());
        } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
            if let Some(extinf) = current_extinf.take() {
                blocks.push((extinf, trimmed.to_string()));
            }
        }
    }

    let channels: Vec<Channel> = blocks
        .par_iter()
        .enumerate()
        .filter_map(|(idx, (extinf, url))| parse_extinf_block(idx, extinf, url))
        .collect();

    Ok(channels)
}

/// Fetch an M3U playlist from a URL using streaming HTTP.
/// Streams the response to a temp file first to avoid holding the whole
/// response body in memory -- critical for large (100 MB+) playlists.
pub async fn fetch_and_parse_m3u(url: &str) -> Result<Vec<Channel>, M3uError> {
    use tokio::io::AsyncWriteExt;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()?;

    let mut response = client.get(url).send().await?.error_for_status()?;

    let tmp_path = std::env::temp_dir().join(format!("mvp_m3u_{}.tmp", std::process::id()));

    let mut file = tokio::fs::File::create(&tmp_path)
        .await
        .map_err(M3uError::Io)?;

    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk).await.map_err(M3uError::Io)?;
    }
    file.flush().await.map_err(M3uError::Io)?;
    drop(file);

    let path = tmp_path.clone();
    let result = tokio::task::spawn_blocking(move || parse_m3u_file(&path))
        .await
        .map_err(|e| M3uError::Parse {
            line: 0,
            message: format!("task join error: {e}"),
        })?;

    let _ = std::fs::remove_file(&tmp_path);

    result
}

/// Strip UTF-8 BOM if present (common in Windows-created M3U files).
fn strip_bom(s: &str) -> &str {
    s.strip_prefix('\u{FEFF}').unwrap_or(s)
}

fn collect_blocks_from_lines<'a>(lines: &[&'a str]) -> Vec<(&'a str, &'a str)> {
    let mut blocks = Vec::new();
    let mut i = 1;
    while i < lines.len() {
        let line = lines[i].trim();
        if line.starts_with("#EXTINF:") {
            if i + 1 < lines.len() {
                let url_line = lines[i + 1].trim();
                if !url_line.is_empty() && !url_line.starts_with('#') {
                    blocks.push((line, url_line));
                    i += 2;
                    continue;
                }
            }
        }
        i += 1;
    }
    blocks
}

fn parse_extinf_block(index: usize, extinf: &str, url: &str) -> Option<Channel> {
    let after_extinf = extinf.strip_prefix("#EXTINF:")?;

    let comma_pos = after_extinf.rfind(',')?;
    let attrs_part = &after_extinf[..comma_pos];
    let name = after_extinf[comma_pos + 1..].trim().to_string();

    let tvg_id = extract_attr(attrs_part, "tvg-id");
    let tvg_name = extract_attr(attrs_part, "tvg-name");
    let logo_url = extract_attr(attrs_part, "tvg-logo");
    let group_title = extract_attr(attrs_part, "group-title").unwrap_or_default();

    Some(Channel {
        id: format!("ch-{index}"),
        name,
        url: url.to_string(),
        logo_url,
        group_title,
        tvg_id,
        tvg_name,
        is_favorite: false,
    })
}

fn extract_attr(s: &str, key: &str) -> Option<String> {
    let pattern = format!("{key}=\"");
    let start = s.find(&pattern)? + pattern.len();
    let end = s[start..].find('"')? + start;
    let val = s[start..end].trim().to_string();
    if val.is_empty() {
        None
    } else {
        Some(val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;

    #[test]
    fn test_parse_basic_m3u() {
        let content = r#"#EXTM3U
#EXTINF:-1 tvg-id="ch1" tvg-name="Channel 1" tvg-logo="http://logo.png" group-title="News",Channel 1
http://stream.example.com/ch1
#EXTINF:-1 tvg-id="ch2" group-title="Sports",Channel 2
http://stream.example.com/ch2
"#;
        let channels = parse_m3u(content).unwrap();
        assert_eq!(channels.len(), 2);
        assert_eq!(channels[0].name, "Channel 1");
        assert_eq!(channels[0].group_title, "News");
        assert_eq!(channels[0].logo_url, Some("http://logo.png".into()));
        assert_eq!(channels[1].name, "Channel 2");
        assert_eq!(channels[1].group_title, "Sports");
    }

    #[test]
    fn test_parse_with_bom() {
        let content = "\u{FEFF}#EXTM3U\n#EXTINF:-1 group-title=\"Test\",BOM Channel\nhttp://bom.com\n";
        let channels = parse_m3u(content).unwrap();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].name, "BOM Channel");
    }

    #[test]
    fn test_parse_via_reader() {
        let content = "#EXTM3U\n#EXTINF:-1 group-title=\"Test\",Channel A\nhttp://a.com\n";
        let reader = BufReader::new(content.as_bytes());
        let channels = parse_m3u_reader(reader).unwrap();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].name, "Channel A");
    }

    #[test]
    fn test_reader_with_bom() {
        let content = "\u{FEFF}#EXTM3U\n#EXTINF:-1 group-title=\"Test\",BOM Reader\nhttp://b.com\n";
        let reader = BufReader::new(content.as_bytes());
        let channels = parse_m3u_reader(reader).unwrap();
        assert_eq!(channels.len(), 1);
    }

    #[test]
    fn test_missing_header() {
        let content = "#EXTINF:-1,Channel\nhttp://url";
        assert!(parse_m3u(content).is_err());
    }

    #[test]
    fn test_empty_playlist() {
        let content = "#EXTM3U\n";
        let channels = parse_m3u(content).unwrap();
        assert!(channels.is_empty());
    }
}
