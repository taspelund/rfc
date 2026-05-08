use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

use crate::models::{DocumentType, Format};

/// Response from datatracker document API
#[derive(Debug, Deserialize)]
struct DraftInfo {
    rev: Option<String>,
}

/// Client for fetching RFC and draft content
pub struct DocumentFetcher {
    client: Client,
}

impl DocumentFetcher {
    /// Create a new document fetcher with its own HTTP client.
    pub fn new() -> Result<Self> {
        Ok(Self::with_client(super::build_http_client()?))
    }

    /// Create a new document fetcher backed by an existing HTTP client.
    pub fn with_client(client: Client) -> Self {
        Self { client }
    }

    /// Fetch document in the preferred format (text first, fallback to HTML)
    pub async fn fetch(&self, doc: &DocumentType) -> Result<(String, Format)> {
        let doc = self.resolve_draft_version(doc).await?;

        // Try text first
        let text_url = self.text_url(&doc);
        match self.fetch_content(&text_url).await {
            Ok(content) => Ok((content, Format::Text)),
            Err(text_err) => {
                // Fallback to HTML
                let html_url = self.html_url(&doc);
                let content = self.fetch_content(&html_url).await.with_context(|| {
                    format!(
                        "Plain text fetch failed ({}); HTML fallback also failed",
                        text_err
                    )
                })?;
                Ok((content, Format::Html))
            }
        }
    }

    /// Resolve a draft name to include its version number if missing
    async fn resolve_draft_version(&self, doc: &DocumentType) -> Result<DocumentType> {
        match doc {
            DocumentType::Rfc(_) => Ok(doc.clone()),
            DocumentType::Draft(name) => {
                // Check if already has a version number (ends with -NN)
                if Self::has_version_suffix(name) {
                    return Ok(doc.clone());
                }

                // Query datatracker for the latest version
                let url = format!("https://datatracker.ietf.org/doc/{}/doc.json", name);
                let response = self
                    .client
                    .get(&url)
                    .send()
                    .await
                    .context("Failed to query draft info")?;

                if !response.status().is_success() {
                    anyhow::bail!("Draft not found: {}", name);
                }

                let info: DraftInfo = response
                    .json()
                    .await
                    .context("Failed to parse draft info")?;

                match info.rev {
                    Some(rev) => Ok(DocumentType::Draft(format!("{}-{}", name, rev))),
                    None => Ok(doc.clone()),
                }
            }
        }
    }

    /// Check if a draft name already has a version suffix (e.g., -06, -12)
    fn has_version_suffix(name: &str) -> bool {
        // Look for pattern like -NN at the end where NN is digits
        if let Some(last_dash) = name.rfind('-') {
            let suffix = &name[last_dash + 1..];
            !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit())
        } else {
            false
        }
    }

    /// Get the HTML URL for a document
    pub fn html_url(&self, doc: &DocumentType) -> String {
        match doc {
            DocumentType::Rfc(num) => {
                format!("https://www.rfc-editor.org/rfc/rfc{}.html", num)
            }
            DocumentType::Draft(name) => {
                format!("https://datatracker.ietf.org/doc/html/{}", name)
            }
        }
    }

    /// Get the plain text URL for a document
    pub fn text_url(&self, doc: &DocumentType) -> String {
        match doc {
            DocumentType::Rfc(num) => {
                format!("https://www.rfc-editor.org/rfc/rfc{}.txt", num)
            }
            DocumentType::Draft(name) => {
                format!("https://www.ietf.org/archive/id/{}.txt", name)
            }
        }
    }

    /// Fetch content from a URL
    async fn fetch_content(&self, url: &str) -> Result<String> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .context("Failed to fetch document")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to fetch {}: HTTP {}", url, response.status());
        }

        response
            .text()
            .await
            .context("Failed to read document content")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rfc_urls() {
        let editor = DocumentFetcher::new().unwrap();

        assert_eq!(
            editor.html_url(&DocumentType::Rfc(9000)),
            "https://www.rfc-editor.org/rfc/rfc9000.html"
        );
        assert_eq!(
            editor.text_url(&DocumentType::Rfc(9000)),
            "https://www.rfc-editor.org/rfc/rfc9000.txt"
        );
    }

    #[test]
    fn test_draft_urls() {
        let editor = DocumentFetcher::new().unwrap();
        let draft = DocumentType::Draft("draft-ietf-quic-transport-34".to_string());

        assert_eq!(
            editor.html_url(&draft),
            "https://datatracker.ietf.org/doc/html/draft-ietf-quic-transport-34"
        );
        assert_eq!(
            editor.text_url(&draft),
            "https://www.ietf.org/archive/id/draft-ietf-quic-transport-34.txt"
        );
    }

    #[test]
    fn test_has_version_suffix() {
        // Has version suffix
        assert!(DocumentFetcher::has_version_suffix(
            "draft-ietf-quic-transport-34"
        ));
        assert!(DocumentFetcher::has_version_suffix("draft-foo-00"));
        assert!(DocumentFetcher::has_version_suffix("draft-test-123456"));

        // No version suffix
        assert!(!DocumentFetcher::has_version_suffix(
            "draft-ietf-quic-transport"
        ));
        assert!(!DocumentFetcher::has_version_suffix("draft-foo-bar-v2")); // v2 has letter
        assert!(!DocumentFetcher::has_version_suffix("draft-foo-bar-")); // empty suffix
        assert!(!DocumentFetcher::has_version_suffix("draftname")); // no dash
        assert!(!DocumentFetcher::has_version_suffix("")); // empty string
    }
}
