//! Shared "fetch from API, cache content + metadata" pipeline used by both
//! the default view command and the explicit `fetch` subcommand.

use anyhow::Result;
use chrono::Utc;

use crate::api::{DataTrackerClient, DocumentFetcher};
use crate::cache::{CacheManager, CacheMetadata};
use crate::models::{DocumentType, Format};

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

fn html_to_text(html: &str) -> String {
    html2text::from_read(html.as_bytes(), 80).unwrap_or_else(|e| {
        eprintln!(
            "Warning: HTML to text conversion failed ({}), displaying raw HTML",
            e
        );
        html.to_string()
    })
}
