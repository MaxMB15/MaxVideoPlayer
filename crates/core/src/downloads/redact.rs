// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Max Boksem. See NOTICE for additional terms under GPLv3 section 7.

/// Scrub Xtream credentials from any string before it is logged, returned in an
/// error, or emitted in an event. Xtream URLs embed credentials as path
/// segments: `http://host:8080/movie/<user>/<pass>/12345.mkv`. We also redact
/// `username=`/`password=` query parameters used by EPG/auth URLs.
pub fn redact_credentials(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for token in split_keep(input) {
        out.push_str(&redact_token(token));
    }
    out
}

/// Split on whitespace but keep the separators so messages read naturally.
fn split_keep(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut last = 0;
    for (i, c) in input.char_indices() {
        if c.is_whitespace() {
            if last < i {
                parts.push(&input[last..i]);
            }
            parts.push(&input[i..i + c.len_utf8()]);
            last = i + c.len_utf8();
        }
    }
    if last < input.len() {
        parts.push(&input[last..]);
    }
    parts
}

fn redact_token(token: &str) -> String {
    // Query-style credentials.
    let mut t = token.to_string();
    for key in ["username", "password"] {
        t = redact_query_param(&t, key);
    }
    // Path-style Xtream credentials: /movie|series|live/<user>/<pass>/<id>
    for kind in ["movie", "series", "live"] {
        if let Some(redacted) = redact_path_creds(&t, kind) {
            t = redacted;
        }
    }
    t
}

fn redact_query_param(token: &str, key: &str) -> String {
    let needle = format!("{key}=");
    if let Some(start) = token.find(&needle) {
        let value_start = start + needle.len();
        let value_end = token[value_start..]
            .find(['&', ' '])
            .map(|i| value_start + i)
            .unwrap_or(token.len());
        let mut s = String::new();
        s.push_str(&token[..value_start]);
        s.push_str("***");
        s.push_str(&token[value_end..]);
        return s;
    }
    token.to_string()
}

fn redact_path_creds(token: &str, kind: &str) -> Option<String> {
    let marker = format!("/{kind}/");
    let idx = token.find(&marker)?;
    let after = idx + marker.len();
    let rest = &token[after..];
    let mut segs = rest.splitn(3, '/');
    let _user = segs.next()?;
    let _pass = segs.next()?;
    let tail = segs.next().unwrap_or("");
    Some(format!("{}{}***/***/{}", &token[..idx], marker, tail))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_path_credentials() {
        let url = "http://host:8080/movie/john/s3cret/12345.mkv";
        let out = redact_credentials(url);
        assert!(!out.contains("john"));
        assert!(!out.contains("s3cret"));
        assert!(out.contains("12345.mkv"));
        assert!(out.contains("/movie/***/***/"));
    }

    #[test]
    fn redacts_query_credentials() {
        let url = "http://host/xmltv.php?username=john&password=s3cret";
        let out = redact_credentials(url);
        assert!(out.contains("username=***"));
        assert!(out.contains("password=***"));
        assert!(!out.contains("john"));
        assert!(!out.contains("s3cret"));
    }

    #[test]
    fn leaves_clean_text_untouched() {
        let msg = "download failed: connection reset";
        assert_eq!(redact_credentials(msg), msg);
    }
}
