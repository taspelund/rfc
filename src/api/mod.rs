mod datatracker;
mod rfc_editor;

use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::Client;

pub use datatracker::DataTrackerClient;
pub use rfc_editor::DocumentFetcher;

/// Build the shared HTTP client used by every API wrapper.
///
/// All callers want the same user-agent and timeout, so creating a fresh
/// `reqwest::Client` per call would just rebuild the connection pool.
pub fn build_http_client() -> Result<Client> {
    Client::builder()
        .user_agent(concat!("rfc-cli/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(30))
        .build()
        .context("Failed to create HTTP client")
}
