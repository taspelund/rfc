//! Shared "fetch from API, cache content + metadata" pipeline used by both
//! the default view command and the explicit `fetch` subcommand.

use anyhow::Result;
use chrono::Utc;

use crate::api::{build_http_client, DataTrackerClient, DocumentFetcher};
use crate::cache::{CacheManager, CacheMetadata};
use crate::models::{DocumentType, Format};

/// Create a shared HTTP client and split it into fetcher + datatracker.
pub(super) fn setup_http_clients() -> Result<(DocumentFetcher, DataTrackerClient)> {
    let http = build_http_client()?;
    let fetcher = DocumentFetcher::with_client(http.clone());
    let datatracker = DataTrackerClient::with_client(http);
    Ok((fetcher, datatracker))
}

/// Fetch a document and store both its content and metadata in the cache.
/// Metadata fetch failures are non-fatal — the content is still returned.
pub async fn fetch_and_cache(
    doc_type: &DocumentType,
    cache: &CacheManager,
    fetcher: &DocumentFetcher,
    datatracker: &DataTrackerClient,
) -> Result<String> {
    eprintln!("Fetching {}...", doc_type);

    let (content, format) = fetcher.fetch(doc_type).await?;
    let text = match format {
        Format::Text => content,
        Format::Html => {
            eprintln!("Plain text not available, converting from HTML...");
            html_to_text(&content)
        }
    };

    cache.store_document(doc_type, Format::Text, &text)?;

    if let Err(e) = store_metadata(doc_type, cache, datatracker).await {
        eprintln!("Warning: Failed to fetch metadata for {}: {}", doc_type, e);
    }

    Ok(text)
}

async fn store_metadata(
    doc_type: &DocumentType,
    cache: &CacheManager,
    datatracker: &DataTrackerClient,
) -> Result<()> {
    let doc = datatracker.get_document(&doc_type.name()).await?;
    let metadata = CacheMetadata {
        title: doc.title,
        cached_at: Utc::now(),
    };
    cache.store_metadata(doc_type, &metadata)?;
    Ok(())
}

pub(crate) fn html_to_text(html: &str) -> String {
    html2text::from_read(html.as_bytes(), 80).unwrap_or_else(|e| {
        eprintln!(
            "Warning: HTML to text conversion failed ({}), displaying raw HTML",
            e
        );
        html.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_to_text_simple() {
        let text = html_to_text("<p>hello world</p>");
        assert!(text.contains("hello world"), "got: {:?}", text);
    }

    #[test]
    fn html_to_text_empty() {
        let text = html_to_text("");
        assert_eq!(text, "");
    }

    #[test]
    fn html_to_text_unicode() {
        let text = html_to_text("<p>café résumé</p>");
        assert!(text.contains("café"), "got: {:?}", text);
        assert!(text.contains("résumé"), "got: {:?}", text);
    }

    #[test]
    fn html_to_text_line_breaks() {
        let text = html_to_text("<p>line1</p><p>line2</p>");
        assert!(text.contains("line1"), "got: {:?}", text);
        assert!(text.contains("line2"), "got: {:?}", text);
    }
}
