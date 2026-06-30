use anyhow::Result;

use crate::cache::CacheManager;
use crate::models::DocumentType;

use super::fetch_pipeline::{fetch_and_cache, setup_http_clients};

/// Always-fresh fetch: hit the API, cache the result, do not open.
pub async fn run(document: &str) -> Result<()> {
    let doc_type = DocumentType::from_user_input(document);
    let cache = CacheManager::new()?;
    let (fetcher, datatracker) = setup_http_clients()?;

    fetch_and_cache(&doc_type, &cache, &fetcher, &datatracker).await?;
    eprintln!("Cached {}. Use 'rfc {}' to view.", doc_type, doc_type);
    Ok(())
}
