use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

use crate::models::{Document, DocumentType, SearchFilter, SearchResult};

const DATATRACKER_BASE_URL: &str = "https://datatracker.ietf.org";

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
    total_count: Option<u32>,
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

    /// Search for documents matching the query.
    ///
    /// The query is tokenized on whitespace and pushed to the server as much
    /// as possible:
    ///
    /// - the longest token becomes a `title__icontains` filter,
    /// - the second-longest (if any) becomes an `abstract__icontains` filter,
    /// - `type__in=rfc,draft` (or the user's explicit `--rfc`/`--draft`)
    ///   ensures we don't waste payload on slides, agendas, charters, etc.
    ///
    /// Any remaining (3rd+) tokens are AND-ed locally against title+abstract.
    /// This makes word-order-independent searches like "bgp message" work
    /// without the user having to guess the exact phrase, while keeping the
    /// JSON payload (and latency) small.
    pub async fn search(
        &self,
        query: &str,
        filter: SearchFilter,
        limit: u32,
    ) -> Result<SearchResult> {
        let tokens: Vec<String> = query.split_whitespace().map(|t| t.to_lowercase()).collect();

        // Pick the longest token for the title filter, the second-longest for
        // the abstract filter. Falls back to the raw query when there are no
        // whitespace-separated tokens (e.g. empty input).
        let mut by_length: Vec<&str> = tokens.iter().map(String::as_str).collect();
        by_length.sort_by_key(|t| std::cmp::Reverse(t.len()));
        let primary_token = by_length.first().copied().unwrap_or(query);
        let secondary_token = by_length.get(1).copied();

        // Server-side type filter. If the user asked for --rfc or --draft we
        // honor that; otherwise we restrict to rfc+draft so the response
        // doesn't waste rows on slides, charters, reviews, etc.
        let type_filter = filter.api_param().unwrap_or("rfc,draft");

        // Cushion sizing. With both title and abstract filters server-side,
        // multi-token queries are already very selective — asking for the
        // user's limit verbatim is enough. Single-token queries lack the
        // abstract filter, so we keep a small cushion (3x) for the
        // ID-ordering-fallthrough effect we observed in benchmarks.
        let base_limit = limit.max(25);
        let api_limit = if secondary_token.is_some() {
            base_limit
        } else {
            base_limit.saturating_mul(3)
        };

        let mut url = format!(
            "{}/api/v1/doc/document/?title__icontains={}&type__in={}&limit={}&format=json",
            DATATRACKER_BASE_URL,
            urlencoding::encode(primary_token),
            type_filter,
            api_limit
        );
        if let Some(s) = secondary_token {
            url.push_str(&format!("&abstract__icontains={}", urlencoding::encode(s)));
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

        // 3rd+ tokens weren't sent to the API; each must still match locally
        // against title or abstract for the document to be included.
        let extra_tokens: Vec<&str> = by_length.iter().copied().skip(2).collect();

        let matches_extra_tokens = |doc: &ApiDocument| -> bool {
            if extra_tokens.is_empty() {
                return true;
            }
            let title_lc = doc.title.to_lowercase();
            let abstract_lc = doc.abstract_text.as_deref().unwrap_or("").to_lowercase();
            extra_tokens
                .iter()
                .all(|tok| title_lc.contains(tok) || abstract_lc.contains(tok))
        };

        // Filter to only RFCs and drafts that match all query tokens, then take
        // up to the requested limit.
        let documents: Vec<Document> = search_response
            .objects
            .into_iter()
            .filter(|doc| Self::is_rfc_or_draft(&doc.name))
            .filter(matches_extra_tokens)
            .map(|doc| self.convert_api_document(doc))
            .take(limit as usize)
            .collect();

        // The API's total_count reflects all server-side filters (title,
        // abstract, type) — it's accurate when we have no further local
        // filtering to do. With 3+ tokens we filter locally too, so drop it.
        let total_count = if extra_tokens.is_empty() {
            search_response.meta.total_count
        } else {
            None
        };

        Ok(SearchResult {
            documents,
            has_more: search_response.meta.next.is_some(),
            total_count,
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

    /// Get a single document by name
    pub async fn get_document(&self, name: &str) -> Result<Document> {
        let url = format!(
            "{}/api/v1/doc/document/{}/?format=json",
            DATATRACKER_BASE_URL, name
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch document metadata")?;

        if !response.status().is_success() {
            anyhow::bail!("Document not found: {}", name);
        }

        let api_doc: ApiDocument = response
            .json()
            .await
            .context("Failed to parse document metadata")?;

        Ok(self.convert_api_document(api_doc))
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
