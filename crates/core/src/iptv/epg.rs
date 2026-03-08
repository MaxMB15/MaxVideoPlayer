use chrono::NaiveDateTime;
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EpgError {
    #[error("XML parse error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("invalid date format: {0}")]
    DateParse(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpgProgram {
    pub channel_id: String,
    pub title: String,
    pub description: String,
    pub start_time: String,
    pub end_time: String,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpgChannel {
    pub id: String,
    pub display_name: String,
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpgData {
    pub channels: Vec<EpgChannel>,
    pub programs: Vec<EpgProgram>,
}

/// Parse XMLTV EPG data from a string.
pub fn parse_epg(xml: &str) -> Result<EpgData, EpgError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut channels = Vec::new();
    let mut programs = Vec::new();
    let mut buf = Vec::new();

    // Parsing state
    let mut in_channel = false;
    let mut in_programme = false;
    let mut current_channel_id = String::new();
    let mut current_display_name = String::new();
    let mut current_icon_url: Option<String> = None;
    let mut current_prog = EpgProgram {
        channel_id: String::new(),
        title: String::new(),
        description: String::new(),
        start_time: String::new(),
        end_time: String::new(),
        category: None,
    };
    let mut current_tag = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "channel" => {
                        in_channel = true;
                        current_channel_id = extract_xml_attr(&e, "id").unwrap_or_default();
                        current_display_name.clear();
                        current_icon_url = None;
                    }
                    "programme" => {
                        in_programme = true;
                        let ch = extract_xml_attr(&e, "channel").unwrap_or_default();
                        let start = extract_xml_attr(&e, "start").unwrap_or_default();
                        let stop = extract_xml_attr(&e, "stop").unwrap_or_default();
                        current_prog = EpgProgram {
                            channel_id: ch,
                            title: String::new(),
                            description: String::new(),
                            start_time: normalize_epg_time(&start),
                            end_time: normalize_epg_time(&stop),
                            category: None,
                        };
                    }
                    _ => {}
                }
                current_tag = tag;
            }
            Ok(Event::Empty(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if in_channel && tag == "icon" {
                    current_icon_url = extract_xml_attr(&e, "src");
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if in_channel && current_tag == "display-name" {
                    current_display_name = text;
                } else if in_programme {
                    match current_tag.as_str() {
                        "title" => current_prog.title = text,
                        "desc" => current_prog.description = text,
                        "category" => current_prog.category = Some(text),
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "channel" => {
                        in_channel = false;
                        channels.push(EpgChannel {
                            id: std::mem::take(&mut current_channel_id),
                            display_name: std::mem::take(&mut current_display_name),
                            icon_url: current_icon_url.take(),
                        });
                    }
                    "programme" => {
                        in_programme = false;
                        programs.push(std::mem::replace(
                            &mut current_prog,
                            EpgProgram {
                                channel_id: String::new(),
                                title: String::new(),
                                description: String::new(),
                                start_time: String::new(),
                                end_time: String::new(),
                                category: None,
                            },
                        ));
                    }
                    _ => {}
                }
                current_tag.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(EpgError::Xml(e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(EpgData { channels, programs })
}

fn extract_xml_attr(e: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == key.as_bytes() {
            return Some(String::from_utf8_lossy(&attr.value).to_string());
        }
    }
    None
}

/// Normalize XMLTV timestamps like "20260304120000 +0000" into ISO-ish strings.
fn normalize_epg_time(raw: &str) -> String {
    let cleaned = raw.split_whitespace().next().unwrap_or(raw);
    if cleaned.len() >= 14 {
        if let Ok(dt) = NaiveDateTime::parse_from_str(&cleaned[..14], "%Y%m%d%H%M%S") {
            return dt.format("%Y-%m-%dT%H:%M:%S").to_string();
        }
    }
    raw.to_string()
}

/// Fetch XMLTV EPG data from a URL and parse it.
pub async fn fetch_and_parse_epg(url: &str) -> Result<EpgData, EpgError> {
    let body = reqwest::get(url).await?.text().await?;
    parse_epg(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_epg() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tv>
  <channel id="ch1">
    <display-name>Channel 1</display-name>
    <icon src="http://logo.png"/>
  </channel>
  <programme start="20260304120000 +0000" stop="20260304130000 +0000" channel="ch1">
    <title>News at Noon</title>
    <desc>Daily news broadcast</desc>
    <category>News</category>
  </programme>
</tv>"#;

        let data = parse_epg(xml).unwrap();
        assert_eq!(data.channels.len(), 1);
        assert_eq!(data.channels[0].display_name, "Channel 1");
        assert_eq!(data.programs.len(), 1);
        assert_eq!(data.programs[0].title, "News at Noon");
        assert_eq!(data.programs[0].start_time, "2026-03-04T12:00:00");
    }

    #[test]
    fn test_normalize_time() {
        assert_eq!(
            normalize_epg_time("20260304120000 +0000"),
            "2026-03-04T12:00:00"
        );
    }

    // --- Edge cases ---

    #[test]
    fn test_empty_tv_block() {
        let xml = r#"<?xml version="1.0"?><tv></tv>"#;
        let data = parse_epg(xml).unwrap();
        assert!(data.channels.is_empty());
        assert!(data.programs.is_empty());
    }

    #[test]
    fn test_programme_without_desc_or_category() {
        // desc and category elements are optional — missing ones should be empty/None
        let xml = r#"<?xml version="1.0"?>
<tv>
  <programme start="20260101120000 +0000" stop="20260101130000 +0000" channel="ch1">
    <title>Untitled Show</title>
  </programme>
</tv>"#;
        let data = parse_epg(xml).unwrap();
        assert_eq!(data.programs.len(), 1);
        assert_eq!(data.programs[0].title, "Untitled Show");
        assert_eq!(data.programs[0].description, "", "missing desc should be empty");
        assert!(data.programs[0].category.is_none(), "missing category should be None");
    }

    #[test]
    fn test_multiple_programmes_same_channel() {
        let xml = r#"<?xml version="1.0"?>
<tv>
  <channel id="ch1"><display-name>Channel 1</display-name></channel>
  <programme start="20260101120000 +0000" stop="20260101130000 +0000" channel="ch1">
    <title>Morning Show</title>
  </programme>
  <programme start="20260101130000 +0000" stop="20260101140000 +0000" channel="ch1">
    <title>Midday News</title>
  </programme>
  <programme start="20260101140000 +0000" stop="20260101150000 +0000" channel="ch1">
    <title>Afternoon Drama</title>
  </programme>
</tv>"#;
        let data = parse_epg(xml).unwrap();
        assert_eq!(data.channels.len(), 1);
        assert_eq!(data.programs.len(), 3);
        assert!(data.programs.iter().all(|p| p.channel_id == "ch1"));
        let titles: Vec<&str> = data.programs.iter().map(|p| p.title.as_str()).collect();
        assert!(titles.contains(&"Morning Show"));
        assert!(titles.contains(&"Midday News"));
        assert!(titles.contains(&"Afternoon Drama"));
    }

    #[test]
    fn test_channel_without_icon_has_none() {
        let xml = r#"<?xml version="1.0"?>
<tv>
  <channel id="ch1">
    <display-name>No Icon Channel</display-name>
  </channel>
</tv>"#;
        let data = parse_epg(xml).unwrap();
        assert_eq!(data.channels.len(), 1);
        assert!(data.channels[0].icon_url.is_none());
    }

    #[test]
    fn test_normalize_time_no_timezone() {
        // XMLTV timestamps without a timezone offset should still parse
        assert_eq!(normalize_epg_time("20260304120000"), "2026-03-04T12:00:00");
    }

    #[test]
    fn test_normalize_time_short_returns_raw() {
        // Timestamps shorter than 14 chars can't be parsed — return the raw string
        let raw = "202603";
        assert_eq!(normalize_epg_time(raw), raw);
    }

    #[test]
    fn test_normalize_time_midnight() {
        assert_eq!(normalize_epg_time("20260101000000 +0000"), "2026-01-01T00:00:00");
    }

    #[test]
    fn test_multiple_channels_parsed() {
        let xml = r#"<?xml version="1.0"?>
<tv>
  <channel id="ch1"><display-name>BBC One</display-name><icon src="http://bbc.png"/></channel>
  <channel id="ch2"><display-name>CNN</display-name></channel>
  <channel id="ch3"><display-name>ESPN</display-name><icon src="http://espn.png"/></channel>
</tv>"#;
        let data = parse_epg(xml).unwrap();
        assert_eq!(data.channels.len(), 3);
        let ch_bbc = data.channels.iter().find(|c| c.id == "ch1").unwrap();
        assert_eq!(ch_bbc.display_name, "BBC One");
        assert_eq!(ch_bbc.icon_url.as_deref(), Some("http://bbc.png"));
        let ch_cnn = data.channels.iter().find(|c| c.id == "ch2").unwrap();
        assert!(ch_cnn.icon_url.is_none());
    }
}
