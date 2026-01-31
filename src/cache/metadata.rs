use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Metadata associated with a cached document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMetadata {
    /// Document title
    pub title: String,
    /// When the document was cached
    pub cached_at: DateTime<Utc>,
}
