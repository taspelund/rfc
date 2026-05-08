use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

use crate::models::{DocumentType, Format};

#[derive(Debug, Deserialize)]
struct DraftInfo {
    rev: Option<String>,
}

/// Fetches RFC and draft document content (HTML or plain text).
///
/// Talks primarily to rfc-editor.org and ietf.org/archive, with a side
/// trip to datatracker.ietf.org to resolve `-NN` version suffixes for
/// drafts the user supplied unversioned.
pub struct DocumentFetcher {
    client: Client,
}

impl DocumentFetcher {
    /// Build a fetcher with a freshly-constructed HTTP client.
    pub fn new() -> Result<Self> {
        Ok(Self::with_client(super::build_http_client()?))
    }

    /// Build a fetcher that reuses an existing HTTP client. Lets a single
    /// client back both this and `DataTrackerClient` so we don't pay for
    /// two connection pools per command invocation.
    pub fn with_client(client: Client) -> Self {
        Self { client }
    }

    /// Fetch a document, preferring plain text and falling back to HTML.
    ///
    /// Drafts without a version suffix are resolved to their latest
    /// revision via datatracker before fetching.
    pub async fn fetch(&self, doc: &DocumentType) -> Result<(String, Format)> {
        let doc = self.resolve_draft_version(doc).await?;

        let text_url = self.text_url(&doc);
        match self.fetch_content(&text_url).await {
            Ok(content) => Ok((content, Format::Text)),
            Err(text_err) => {
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

    /// Resolve a draft name to include its latest version suffix.
    /// RFCs and already-versioned drafts pass through unchanged.
    async fn resolve_draft_version(&self, doc: &DocumentType) -> Result<DocumentType> {
        match doc {
            DocumentType::Rfc(_) => Ok(doc.clone()),
            DocumentType::Draft(name) => {
                if Self::has_version_suffix(name) {
                    return Ok(doc.clone());
                }

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

    /// True when `name` ends in `-` followed by ASCII digits (e.g. `-06`,
    /// `-123456`). Used to detect whether a draft name is already pinned
    /// to a specific revision.
    fn has_version_suffix(name: &str) -> bool {
        if let Some(last_dash) = name.rfind('-') {
            let suffix = &name[last_dash + 1..];
            !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit())
        } else {
            false
        }
    }

    /// HTML URL for a document.
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

    /// Plain-text URL for a document.
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
