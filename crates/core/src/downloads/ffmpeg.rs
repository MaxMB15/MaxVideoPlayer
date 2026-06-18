/// Build the ffmpeg argument vector for a stream-copy download.
/// `dest_part` is the temporary `.part` path written during download.
pub fn build_ffmpeg_args(url: &str, dest_part: &str) -> Vec<String> {
    vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        // Transparent recovery from transient network drops.
        "-reconnect".into(),
        "1".into(),
        "-reconnect_streamed".into(),
        "1".into(),
        "-reconnect_at_eof".into(),
        "1".into(),
        "-reconnect_delay_max".into(),
        "30".into(),
        "-i".into(),
        url.into(),
        "-c".into(),
        "copy".into(),
        "-movflags".into(),
        "+faststart".into(),
        // Machine-readable progress on stdout, one key=value per line.
        "-progress".into(),
        "pipe:1".into(),
        "-nostats".into(),
        "-y".into(),
        dest_part.into(),
    ]
}

/// A parsed snapshot from ffmpeg's `-progress` stream. ffmpeg emits a block of
/// `key=value` lines terminated by `progress=continue` or `progress=end`.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ProgressUpdate {
    pub total_size: Option<i64>,
    pub out_time_ms: Option<i64>,
    pub done: bool,
}

/// Fold a single `key=value` line into an accumulating ProgressUpdate.
/// Returns true when the block is complete (`progress=continue|end`).
pub fn fold_progress_line(acc: &mut ProgressUpdate, line: &str) -> bool {
    let line = line.trim();
    let Some((key, value)) = line.split_once('=') else {
        return false;
    };
    match key {
        "total_size" => acc.total_size = value.parse().ok(),
        "out_time_ms" => acc.out_time_ms = value.parse().ok(),
        "progress" => {
            acc.done = value == "end";
            return true;
        }
        _ => {}
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_use_stream_copy_and_progress_pipe() {
        let args = build_ffmpeg_args("http://h/movie/u/p/1.mkv", "/tmp/1.mkv.part");
        let joined = args.join(" ");
        assert!(joined.contains("-c copy"));
        assert!(joined.contains("-progress pipe:1"));
        assert!(joined.contains("-reconnect 1"));
        assert_eq!(args.last().unwrap(), "/tmp/1.mkv.part");
        // URL is the argument right after -i.
        let i = args.iter().position(|a| a == "-i").unwrap();
        assert_eq!(args[i + 1], "http://h/movie/u/p/1.mkv");
    }

    #[test]
    fn folds_progress_block() {
        let mut acc = ProgressUpdate::default();
        assert!(!fold_progress_line(&mut acc, "total_size=410000000"));
        assert!(!fold_progress_line(&mut acc, "out_time_ms=41000000"));
        let complete = fold_progress_line(&mut acc, "progress=continue");
        assert!(complete);
        assert_eq!(acc.total_size, Some(410_000_000));
        assert_eq!(acc.out_time_ms, Some(41_000_000));
        assert!(!acc.done);
    }

    #[test]
    fn detects_end() {
        let mut acc = ProgressUpdate::default();
        assert!(fold_progress_line(&mut acc, "progress=end"));
        assert!(acc.done);
    }
}
