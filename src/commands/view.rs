use anyhow::Result;

use crate::api::{DataTrackerClient, DocumentFetcher};
use crate::cache::CacheManager;
use crate::models::{DocumentType, Format};

use super::fetch_pipeline::fetch_and_cache;
use super::viewer;

/// Default-path command: cache-or-fetch then open in a viewer.
pub async fn run(document: &str, open_with: Option<&str>) -> Result<()> {
    let doc_type = DocumentType::from_user_input(document);
    let cache = CacheManager::new()?;

    let content = match cache.get_document(&doc_type, Format::Text) {
        Some(cached) => {
            eprintln!("Using cached copy of {}", doc_type);
            cached
        }
        None => {
            let http = crate::api::build_http_client()?;
            let fetcher = DocumentFetcher::with_client(http.clone());
            let datatracker = DataTrackerClient::with_client(http);
            fetch_and_cache(&doc_type, &cache, &fetcher, &datatracker).await?
        }
    };

    viewer::open(&content, open_with)?;
    Ok(())
}
