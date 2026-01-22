use anyhow::{Context, Result};

use crate::api::{DataTrackerClient, DocumentFetcher};
use crate::cache::CacheManager;
use crate::models::{DocumentType, Format};

use super::fetch_pipeline::fetch_and_cache;
use super::viewer;

/// Default-path command: cache-or-fetch then open in a viewer.
pub async fn run(document: &str, open_with: Option<&str>, web: bool) -> Result<()> {
    let doc_type = DocumentType::from_user_input(document);

    if web {
        return open_in_browser(&doc_type);
    }
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

fn open_in_browser(doc_type: &DocumentType) -> Result<()> {
    let url = doc_type.datatracker_url();
    eprintln!("Opening {} in browser...", doc_type);
    opener::open(&url).with_context(|| format!("Failed to open URL: {}", url))?;
    Ok(())
}
