use serde::Deserialize;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct CategorizedGroup {
    pub group_name: String,
    pub super_category: Option<String>,
    pub sort_order: i64,
}

#[derive(Debug, Clone)]
pub struct CategorizationResult {
    pub hierarchy: Vec<CategorizedGroup>,
    pub ungrouped: Vec<String>,
}

#[derive(Debug)]
pub enum GeminiError {
    Parse(String),
    Api(String),
    InvalidResponse(String),
}

impl std::fmt::Display for GeminiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GeminiError::Parse(s) => write!(f, "Parse error: {}", s),
            GeminiError::Api(s) => write!(f, "API error: {}", s),
            GeminiError::InvalidResponse(s) => write!(f, "Invalid response: {}", s),
        }
    }
}

#[derive(Deserialize)]
struct HierarchicalResponse {
    categories: Vec<CategoryEntry>,
    #[serde(default)]
    ungrouped: Vec<String>,
}

#[derive(Deserialize)]
struct CategoryEntry {
    name: String,
    groups: Vec<String>,
}

#[derive(Deserialize)]
struct FlatResponse {
    groups: Vec<String>,
    #[serde(default)]
    ungrouped: Vec<String>,
}

#[derive(Deserialize)]
struct TypeField {
    #[serde(rename = "type")]
    response_type: String,
}

pub fn parse_categorization_response(
    json: &serde_json::Value,
    known_groups: &[&str],
) -> Result<CategorizationResult, GeminiError> {
    let type_field: TypeField = serde_json::from_value(json.clone())
        .map_err(|e| GeminiError::InvalidResponse(format!("missing 'type' field: {}", e)))?;

    let known_set: HashSet<&str> = known_groups.iter().copied().collect();
    let mut seen: HashSet<String> = HashSet::new();
    let mut hierarchy = Vec::new();
    let mut ungrouped = Vec::new();

    match type_field.response_type.as_str() {
        "hierarchical" => {
            let resp: HierarchicalResponse = serde_json::from_value(json.clone())
                .map_err(|e| GeminiError::InvalidResponse(e.to_string()))?;

            let mut order = 0i64;
            for cat in &resp.categories {
                for group_name in &cat.groups {
                    if known_set.contains(group_name.as_str()) && seen.insert(group_name.clone()) {
                        hierarchy.push(CategorizedGroup {
                            group_name: group_name.clone(),
                            super_category: Some(cat.name.clone()),
                            sort_order: order,
                        });
                        order += 100;
                    }
                }
            }
            for name in &resp.ungrouped {
                if known_set.contains(name.as_str()) && seen.insert(name.clone()) {
                    ungrouped.push(name.clone());
                }
            }
        }
        "flat" => {
            let resp: FlatResponse = serde_json::from_value(json.clone())
                .map_err(|e| GeminiError::InvalidResponse(e.to_string()))?;

            let mut order = 0i64;
            for group_name in &resp.groups {
                if known_set.contains(group_name.as_str()) && seen.insert(group_name.clone()) {
                    hierarchy.push(CategorizedGroup {
                        group_name: group_name.clone(),
                        super_category: None,
                        sort_order: order,
                    });
                    order += 100;
                }
            }
            for name in &resp.ungrouped {
                if known_set.contains(name.as_str()) && seen.insert(name.clone()) {
                    ungrouped.push(name.clone());
                }
            }
        }
        other => return Err(GeminiError::InvalidResponse(format!("unknown type: {}", other))),
    }

    // Add any known groups not mentioned in the response to ungrouped
    for name in known_groups {
        if !seen.contains(*name) {
            ungrouped.push(name.to_string());
        }
    }

    Ok(CategorizationResult { hierarchy, ungrouped })
}

pub fn build_categorization_prompt(
    content_type: &str,
    groups_with_samples: &[(&str, Vec<&str>)],
) -> String {
    let mut prompt = format!(
        "Organize these IPTV channel groups into a navigable hierarchy.\n\
         Content type: {}\n\n\
         Groups (with sample channels):\n",
        content_type
    );
    for (group_name, samples) in groups_with_samples {
        let sample_str = samples.join(", ");
        prompt.push_str(&format!("- \"{}\" → {}\n", group_name, sample_str));
    }
    prompt.push_str(
        "\nRules:\n\
         - Create parent categories ONLY when they meaningfully reduce navigation complexity\n\
         - If groups are already well-organized, return them flat without parents\n\
         - Use clear, short category names\n\
         - Every group must appear exactly once\n\
         - Return JSON matching one of these two schemas:\n\n\
         Hierarchical (when parent categories are useful):\n\
         {\"type\": \"hierarchical\", \"categories\": [{\"name\": \"...\", \"groups\": [\"...\"]}], \"ungrouped\": [\"...\"]}\n\n\
         Flat (when groups stand on their own):\n\
         {\"type\": \"flat\", \"groups\": [\"...\"], \"ungrouped\": [\"...\"]}\n"
    );
    prompt
}

pub fn build_categorization_prompt_no_groups(
    content_type: &str,
    sample_titles: &[&str],
) -> String {
    let mut prompt = format!(
        "Organize these IPTV content titles into navigable groups.\n\
         Content type: {}\n\n\
         Content titles (sample):\n",
        content_type
    );
    for title in sample_titles {
        prompt.push_str(&format!("- {}\n", title));
    }
    prompt.push_str(
        "\nRules:\n\
         - Create logical genre/theme groups from the titles\n\
         - Optionally create parent categories if the number of groups warrants it\n\
         - Use clear, short names\n\
         - Return JSON matching one of these two schemas:\n\n\
         Hierarchical (when parent categories are useful):\n\
         {\"type\": \"hierarchical\", \"categories\": [{\"name\": \"...\", \"groups\": [\"...\"]}], \"ungrouped\": [\"...\"]}\n\n\
         Flat (when groups stand on their own):\n\
         {\"type\": \"flat\", \"groups\": [\"...\"], \"ungrouped\": [\"...\"]}\n"
    );
    prompt
}

pub async fn call_gemini(api_key: &str, prompt: &str) -> Result<serde_json::Value, GeminiError> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={}",
        api_key
    );
    let body = serde_json::json!({
        "contents": [{ "parts": [{ "text": prompt }] }],
        "generationConfig": {
            "responseMimeType": "application/json"
        }
    });

    let client = reqwest::Client::new();
    let response = client.post(&url).json(&body).send().await
        .map_err(|e| GeminiError::Api(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(GeminiError::Api(format!("HTTP {}: {}", status, text)));
    }

    let resp_json: serde_json::Value = response.json().await
        .map_err(|e| GeminiError::Parse(e.to_string()))?;

    let text = resp_json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .ok_or_else(|| GeminiError::Parse("no text in response".into()))?;

    let parsed: serde_json::Value = serde_json::from_str(text)
        .map_err(|e| GeminiError::Parse(format!("LLM returned invalid JSON: {}", e)))?;

    Ok(parsed)
}

pub async fn test_gemini_key(api_key: &str) -> Result<bool, GeminiError> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={}",
        api_key
    );
    let client = reqwest::Client::new();
    let response = client.get(&url).send().await
        .map_err(|e| GeminiError::Api(e.to_string()))?;
    Ok(response.status().is_success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hierarchical_response() {
        let json = serde_json::json!({
            "type": "hierarchical",
            "categories": [
                { "name": "United States", "groups": ["US: Sports", "US: News"] },
                { "name": "United Kingdom", "groups": ["UK: Drama"] }
            ],
            "ungrouped": ["Misc"]
        });
        let result = parse_categorization_response(&json, &["US: Sports", "US: News", "UK: Drama", "Misc"]).unwrap();
        assert_eq!(result.hierarchy.len(), 3);
        assert_eq!(result.ungrouped, vec!["Misc"]);
        let sports = result.hierarchy.iter().find(|h| h.group_name == "US: Sports").unwrap();
        assert_eq!(sports.super_category.as_deref(), Some("United States"));
    }

    #[test]
    fn test_parse_flat_response() {
        let json = serde_json::json!({
            "type": "flat",
            "groups": ["Sports", "News", "Kids"],
            "ungrouped": []
        });
        let result = parse_categorization_response(&json, &["Sports", "News", "Kids"]).unwrap();
        assert_eq!(result.hierarchy.len(), 3);
        assert!(result.hierarchy.iter().all(|h| h.super_category.is_none()));
    }

    #[test]
    fn test_parse_drops_unknown_groups_adds_missing() {
        let json = serde_json::json!({
            "type": "flat",
            "groups": ["Sports", "FakeGroup"],
            "ungrouped": []
        });
        let known = &["Sports", "News"];
        let result = parse_categorization_response(&json, known).unwrap();
        assert!(!result.hierarchy.iter().any(|h| h.group_name == "FakeGroup"));
        assert!(result.ungrouped.contains(&"News".to_string()));
    }

    #[test]
    fn test_build_categorization_prompt_with_groups() {
        let groups_with_samples = vec![
            ("US: Sports", vec!["ESPN HD", "Fox Sports 1"]),
            ("UK: Drama", vec!["BBC One", "ITV"]),
        ];
        let prompt = build_categorization_prompt("live", &groups_with_samples);
        assert!(prompt.contains("Content type: live"));
        assert!(prompt.contains("US: Sports"));
        assert!(prompt.contains("ESPN HD"));
        assert!(prompt.contains(r#""type": "hierarchical""#));
        assert!(prompt.contains(r#""type": "flat""#));
    }
}
