use anyhow::Result;

use crate::api::{DataTrackerClient, DocumentFetcher};
use crate::cache::CacheManager;
use crate::models::DocumentType;

use super::fetch_pipeline::fetch_and_cache;

/// Always-fresh fetch: hit the API, cache the result, do not open.
pub async fn run(document: &str) -> Result<()> {
    let doc_type = DocumentType::from_user_input(document);
    let cache = CacheManager::new()?;
    let http = crate::api::build_http_client()?;
    let fetcher = DocumentFetcher::with_client(http.clone());
    let datatracker = DataTrackerClient::with_client(http);

    fetch_and_cache(&doc_type, &cache, &fetcher, &datatracker).await?;
    eprintln!("Cached {}. Use 'rfc {}' to view.", doc_type, doc_type);
    Ok(())
}
