//! Library backing the `rfc` CLI. Exposes the API clients, cache layer,
//! and document models so the binary can stay a thin dispatcher.

pub mod api;
pub mod cache;
pub mod commands;
pub mod models;

pub use api::{DataTrackerClient, DocumentFetcher};
pub use cache::{CacheManager, CacheMetadata, CachedDocument};
pub use models::{Document, DocumentType, Format, SearchFilter, SearchResult};
