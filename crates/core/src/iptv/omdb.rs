use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OmdbData {
    pub title: String,
    pub year: Option<String>,
    pub rated: Option<String>,
    pub runtime: Option<String>,
    pub genre: Option<String>,
    pub director: Option<String>,
    pub actors: Option<String>,
    pub plot: Option<String>,
    pub poster_url: Option<String>,
    pub imdb_rating: Option<String>,
    pub rotten_tomatoes: Option<String>,
}

#[derive(Debug, Error)]
pub enum OmdbError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("OMDB API error: {0}")]
    Api(String),
    #[error("Parse error: {0}")]
    Parse(String),
}

/// Raw OMDB API response shape (for deserialization only).
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OmdbApiResponse {
    #[serde(rename = "Response")]
    response: String,
    #[serde(rename = "Error")]
    error: Option<String>,
    #[serde(rename = "Title")]
    title: Option<String>,
    #[serde(rename = "Year")]
    year: Option<String>,
    #[serde(rename = "Rated")]
    rated: Option<String>,
    #[serde(rename = "Runtime")]
    runtime: Option<String>,
    #[serde(rename = "Genre")]
    genre: Option<String>,
    #[serde(rename = "Director")]
    director: Option<String>,
    #[serde(rename = "Actors")]
    actors: Option<String>,
    #[serde(rename = "Plot")]
    plot: Option<String>,
    #[serde(rename = "Poster")]
    poster: Option<String>,
    #[serde(rename = "imdbRating")]
    imdb_rating: Option<String>,
    #[serde(rename = "Ratings")]
    ratings: Option<Vec<OmdbRating>>,
}

#[derive(Debug, Deserialize)]
struct OmdbRating {
    #[serde(rename = "Source")]
    source: String,
    #[serde(rename = "Value")]
    value: String,
}

/// Convert an OMDB "N/A" string to None.
fn na_to_none(s: Option<String>) -> Option<String> {
    s.and_then(|v| if v == "N/A" || v.is_empty() { None } else { Some(v) })
}

/// Extract the Rotten Tomatoes rating from the Ratings array.
fn extract_rotten_tomatoes(ratings: Option<Vec<OmdbRating>>) -> Option<String> {
    ratings?.into_iter().find(|r| r.source == "Rotten Tomatoes").map(|r| r.value)
}

/// Parse an OMDB API JSON response (as serde_json::Value) into OmdbData.
/// Separated from the HTTP call so it can be unit-tested without network access.
pub fn parse_omdb_response(json: serde_json::Value) -> Result<OmdbData, OmdbError> {
    let resp: OmdbApiResponse = serde_json::from_value(json)
        .map_err(|e| OmdbError::Parse(e.to_string()))?;

    if resp.response != "True" {
        return Err(OmdbError::Api(resp.error.unwrap_or_else(|| "Unknown OMDB error".into())));
    }

    let title = resp.title.ok_or_else(|| OmdbError::Parse("Missing title".into()))?;

    Ok(OmdbData {
        title,
        year: na_to_none(resp.year),
        rated: na_to_none(resp.rated),
        runtime: na_to_none(resp.runtime),
        genre: na_to_none(resp.genre),
        director: na_to_none(resp.director),
        actors: na_to_none(resp.actors),
        plot: na_to_none(resp.plot),
        poster_url: na_to_none(resp.poster),
        imdb_rating: na_to_none(resp.imdb_rating),
        rotten_tomatoes: extract_rotten_tomatoes(resp.ratings),
    })
}

/// Fetch OMDB data for a title. `content_type` should be `"movie"` or `"series"`.
pub async fn fetch_omdb(title: &str, content_type: &str, api_key: &str) -> Result<OmdbData, OmdbError> {
    // Client constructed per-call; acceptable for low-frequency on-demand OMDB fetches.
    // A shared client can be introduced if this path becomes high-frequency.
    let client = reqwest::Client::new();
    let response = client
        .get("https://www.omdbapi.com/")
        .query(&[("t", title), ("type", content_type), ("apikey", api_key)])
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;
    parse_omdb_response(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn full_response() -> serde_json::Value {
        json!({
            "Response": "True",
            "Title": "The Dark Knight",
            "Year": "2008",
            "Rated": "PG-13",
            "Runtime": "152 min",
            "Genre": "Action, Crime, Drama",
            "Director": "Christopher Nolan",
            "Actors": "Christian Bale, Heath Ledger",
            "Plot": "Batman raises the stakes in his war on crime.",
            "Poster": "https://example.com/poster.jpg",
            "imdbRating": "9.0",
            "Ratings": [
                { "Source": "Internet Movie Database", "Value": "9.0/10" },
                { "Source": "Rotten Tomatoes", "Value": "94%" },
                { "Source": "Metacritic", "Value": "84/100" }
            ]
        })
    }

    #[test]
    fn test_parse_full_response() {
        let data = parse_omdb_response(full_response()).unwrap();
        assert_eq!(data.title, "The Dark Knight");
        assert_eq!(data.year.as_deref(), Some("2008"));
        assert_eq!(data.rated.as_deref(), Some("PG-13"));
        assert_eq!(data.runtime.as_deref(), Some("152 min"));
        assert_eq!(data.genre.as_deref(), Some("Action, Crime, Drama"));
        assert_eq!(data.director.as_deref(), Some("Christopher Nolan"));
        assert_eq!(data.actors.as_deref(), Some("Christian Bale, Heath Ledger"));
        assert!(data.plot.is_some());
        assert_eq!(data.poster_url.as_deref(), Some("https://example.com/poster.jpg"));
        assert_eq!(data.imdb_rating.as_deref(), Some("9.0"));
        assert_eq!(data.rotten_tomatoes.as_deref(), Some("94%"));
    }

    #[test]
    fn test_na_values_become_none() {
        let response = json!({
            "Response": "True",
            "Title": "Some Movie",
            "Year": "N/A",
            "Rated": "N/A",
            "Runtime": "N/A",
            "Genre": "N/A",
            "Director": "N/A",
            "Actors": "N/A",
            "Plot": "N/A",
            "Poster": "N/A",
            "imdbRating": "N/A",
            "Ratings": []
        });
        let data = parse_omdb_response(response).unwrap();
        assert_eq!(data.title, "Some Movie");
        assert!(data.year.is_none());
        assert!(data.rated.is_none());
        assert!(data.runtime.is_none());
        assert!(data.genre.is_none());
        assert!(data.director.is_none());
        assert!(data.actors.is_none());
        assert!(data.plot.is_none());
        assert!(data.poster_url.is_none());
        assert!(data.imdb_rating.is_none());
        assert!(data.rotten_tomatoes.is_none());
    }

    #[test]
    fn test_api_error_response() {
        let response = json!({
            "Response": "False",
            "Error": "Movie not found!"
        });
        let err = parse_omdb_response(response).unwrap_err();
        assert!(matches!(err, OmdbError::Api(_)));
        assert!(err.to_string().contains("Movie not found!"));
    }

    #[test]
    fn test_api_error_without_message() {
        let response = json!({
            "Response": "False"
        });
        let err = parse_omdb_response(response).unwrap_err();
        assert!(matches!(err, OmdbError::Api(_)));
    }

    #[test]
    fn test_no_rotten_tomatoes_in_ratings() {
        let response = json!({
            "Response": "True",
            "Title": "Some Movie",
            "Ratings": [
                { "Source": "Internet Movie Database", "Value": "7.5/10" }
            ]
        });
        let data = parse_omdb_response(response).unwrap();
        assert!(data.rotten_tomatoes.is_none());
    }

    #[test]
    fn test_missing_ratings_field() {
        let response = json!({
            "Response": "True",
            "Title": "Some Movie"
        });
        let data = parse_omdb_response(response).unwrap();
        assert!(data.rotten_tomatoes.is_none());
    }

    #[test]
    fn test_rotten_tomatoes_extracted_when_present() {
        let response = json!({
            "Response": "True",
            "Title": "Great Film",
            "Ratings": [
                { "Source": "Metacritic", "Value": "75/100" },
                { "Source": "Rotten Tomatoes", "Value": "88%" }
            ]
        });
        let data = parse_omdb_response(response).unwrap();
        assert_eq!(data.rotten_tomatoes.as_deref(), Some("88%"));
    }

    #[test]
    fn test_omdb_data_serializes_to_camel_case() {
        let data = OmdbData {
            title: "Test".into(),
            year: Some("2020".into()),
            rated: None,
            runtime: None,
            genre: None,
            director: None,
            actors: None,
            plot: None,
            poster_url: Some("https://example.com/p.jpg".into()),
            imdb_rating: Some("8.5".into()),
            rotten_tomatoes: None,
        };
        let json_str = serde_json::to_string(&data).unwrap();
        assert!(json_str.contains("\"posterUrl\""));
        assert!(json_str.contains("\"imdbRating\""));
        assert!(json_str.contains("\"rottenTomatoes\""));
    }
}
