use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

use crate::models::{Document, DocumentType, SearchFilter, SearchResult};

pub const DATATRACKER_BASE_URL: &str = "https://datatracker.ietf.org";

/// Client for the IETF Datatracker API
pub struct DataTrackerClient {
    client: Client,
}

/// Response from the Datatracker document search API
#[derive(Debug, Deserialize)]
struct SearchResponse {
    meta: SearchMeta,
    objects: Vec<ApiDocument>,
}

#[derive(Debug, Deserialize)]
struct SearchMeta {
    #[serde(default)]
    next: Option<String>,
}

/// Document as returned by the Datatracker API
#[derive(Debug, Deserialize)]
struct ApiDocument {
    name: String,
    title: String,
    #[serde(rename = "abstract")]
    abstract_text: Option<String>,
    pages: Option<u32>,
    #[serde(rename = "time")]
    time: Option<String>,
    #[serde(rename = "std_level")]
    std_level: Option<String>,
    stream: Option<String>,
    #[serde(default)]
    authors: Vec<String>,
}

impl DataTrackerClient {
    /// Create a new DataTracker API client
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: Client::builder()
                .user_agent(concat!("rfc-cli/", env!("CARGO_PKG_VERSION")))
                .timeout(Duration::from_secs(30))
                .build()
                .context("Failed to create HTTP client")?,
        })
    }

    /// Search for documents matching the query
    /// Only returns RFCs and Internet-Drafts (filters out slides, reviews, etc.)
    pub async fn search(
        &self,
        query: &str,
        filter: SearchFilter,
        limit: u32,
    ) -> Result<SearchResult> {
        // Request more results than needed since we filter locally
        // The API returns many document types we don't want (slides, reviews, etc.)
        let api_limit = limit.saturating_mul(5);

        // Search by title (not name) since that's where keywords like "bgp" appear
        let mut url = format!(
            "{}/api/v1/doc/document/?title__icontains={}&limit={}&format=json",
            DATATRACKER_BASE_URL,
            urlencoding::encode(query),
            api_limit
        );

        // Add type filter if specified
        if let Some(type_param) = filter.api_param() {
            url.push_str(&format!("&type={}", type_param));
        }

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to send search request")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Search request to {} failed: HTTP {}",
                url,
                response.status()
            );
        }

        let search_response: SearchResponse = response
            .json()
            .await
            .context("Failed to parse search response")?;

        // Filter to only RFCs and drafts, then take up to the requested limit
        let documents: Vec<Document> = search_response
            .objects
            .into_iter()
            .filter(|doc| Self::is_rfc_or_draft(&doc.name))
            .map(|doc| self.convert_api_document(doc))
            .take(limit as usize)
            .collect();

        let returned_count = documents.len() as u32;

        Ok(SearchResult {
            documents,
            has_more: search_response.meta.next.is_some() || returned_count == limit,
            query: query.to_string(),
            filter,
        })
    }

    /// Check if a document name is an RFC or Internet-Draft
    fn is_rfc_or_draft(name: &str) -> bool {
        name.starts_with("rfc") || name.starts_with("draft-")
    }

    /// Convert an API document to our Document model
    fn convert_api_document(&self, doc: ApiDocument) -> Document {
        let doc_type = self.parse_doc_type(&doc.name);
        let published = doc.time.as_ref().and_then(|t| {
            chrono::DateTime::parse_from_rfc3339(t)
                .ok()
                .map(|dt| dt.with_timezone(&chrono::Utc))
        });

        Document {
            name: doc.name.clone(),
            title: doc.title,
            doc_type,
            abstract_text: doc.abstract_text,
            pages: doc.pages,
            published,
            status: doc.std_level,
            authors: doc.authors,
            stream: doc.stream,
            wg: None,
        }
    }

    /// Parse document type from name
    fn parse_doc_type(&self, name: &str) -> DocumentType {
        if let Some(num_str) = name.strip_prefix("rfc") {
            if let Ok(num) = num_str.parse::<u32>() {
                return DocumentType::Rfc(num);
            }
        }
        DocumentType::Draft(name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_doc_type() {
        let client = DataTrackerClient::new().unwrap();
        assert_eq!(client.parse_doc_type("rfc9000"), DocumentType::Rfc(9000));
        assert_eq!(
            client.parse_doc_type("draft-ietf-quic-transport-34"),
            DocumentType::Draft("draft-ietf-quic-transport-34".to_string())
        );
    }
}
