use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use directories::ProjectDirs;

use crate::cache::CacheMetadata;
use crate::models::{DocumentType, Format};

/// A cached document with optional metadata
#[derive(Debug, Clone)]
pub struct CachedDocument {
    pub doc_type: DocumentType,
    pub metadata: Option<CacheMetadata>,
}

/// Manages local document caching
pub struct CacheManager {
    cache_dir: PathBuf,
}

impl CacheManager {
    /// Create a new cache manager
    pub fn new() -> Result<Self> {
        let cache_dir = Self::default_cache_dir()?;
        fs::create_dir_all(&cache_dir).context("Failed to create cache directory")?;
        Ok(Self { cache_dir })
    }

    /// Create a cache manager with a custom directory
    pub fn with_dir(cache_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&cache_dir).context("Failed to create cache directory")?;
        Ok(Self { cache_dir })
    }

    /// Get the default cache directory
    pub fn default_cache_dir() -> Result<PathBuf> {
        if let Some(proj_dirs) = ProjectDirs::from("", "", "rfc") {
            Ok(proj_dirs.cache_dir().to_path_buf())
        } else {
            // Fallback to home directory
            let home = std::env::var("HOME").context("HOME not set")?;
            Ok(PathBuf::from(home).join(".cache").join("rfc"))
        }
    }

    /// Get cached document content
    pub fn get_document(&self, doc: &DocumentType, format: Format) -> Option<String> {
        let path = self.document_path(doc, format);
        fs::read_to_string(path).ok()
    }

    /// Store document content in cache
    pub fn store_document(&self, doc: &DocumentType, format: Format, content: &str) -> Result<()> {
        let path = self.document_path(doc, format);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("Failed to create document cache directory")?;
        }

        fs::write(&path, content).context("Failed to write document to cache")?;
        Ok(())
    }

    /// Clear all cached documents
    pub fn clear_cache(&self) -> Result<()> {
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir).context("Failed to clear cache")?;
            fs::create_dir_all(&self.cache_dir).context("Failed to recreate cache directory")?;
        }
        Ok(())
    }

    /// Remove a specific document from cache
    /// Removes document content and associated metadata
    /// Returns true if the document was found and removed
    pub fn remove(&self, doc: &DocumentType) -> Result<bool> {
        let html_path = self.document_path(doc, Format::Html);
        let text_path = self.document_path(doc, Format::Text);
        let meta_path = self.metadata_path(doc);

        let mut removed = false;

        if html_path.exists() {
            fs::remove_file(&html_path).context("Failed to remove cached HTML file")?;
            removed = true;
        }

        if text_path.exists() {
            fs::remove_file(&text_path).context("Failed to remove cached text file")?;
            removed = true;
        }

        if meta_path.exists() {
            fs::remove_file(&meta_path).context("Failed to remove cached metadata file")?;
        }

        Ok(removed)
    }

    /// List all cached documents
    pub fn list_cached(&self) -> Vec<DocumentType> {
        let docs_dir = self.cache_dir.join("documents");
        if !docs_dir.exists() {
            return Vec::new();
        }

        let mut documents = Vec::new();

        if let Ok(entries) = fs::read_dir(&docs_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Some(doc_type) = DocumentType::parse(stem) {
                        if !documents.contains(&doc_type) {
                            documents.push(doc_type);
                        }
                    }
                }
            }
        }

        documents
    }

    /// Get the cache directory path
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Get the path for a cached document
    fn document_path(&self, doc: &DocumentType, format: Format) -> PathBuf {
        self.cache_dir
            .join("documents")
            .join(format!("{}.{}", doc.name(), format.extension()))
    }

    /// Get the path for metadata file
    fn metadata_path(&self, doc: &DocumentType) -> PathBuf {
        self.cache_dir
            .join("documents")
            .join(format!("{}.meta", doc.name()))
    }

    /// Get cached metadata for a document
    pub fn get_metadata(&self, doc: &DocumentType) -> Option<CacheMetadata> {
        let path = self.metadata_path(doc);
        let content = fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Store metadata for a document
    pub fn store_metadata(&self, doc: &DocumentType, meta: &CacheMetadata) -> Result<()> {
        let path = self.metadata_path(doc);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("Failed to create metadata directory")?;
        }
        let content = serde_json::to_string_pretty(meta)
            .context("Failed to serialize metadata")?;
        fs::write(path, content).context("Failed to write metadata file")?;
        Ok(())
    }

    /// List cached documents with their metadata
    pub fn list_cached_with_metadata(&self) -> Vec<CachedDocument> {
        self.list_cached()
            .into_iter()
            .map(|doc_type| {
                let metadata = self.get_metadata(&doc_type);
                CachedDocument { doc_type, metadata }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::TempDir;

    fn test_cache() -> (CacheManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let cache = CacheManager::with_dir(temp_dir.path().to_path_buf()).unwrap();
        (cache, temp_dir)
    }

    #[test]
    fn test_store_and_retrieve() {
        let (cache, _temp) = test_cache();
        let doc = DocumentType::Rfc(9000);
        let content = "<html>Test content</html>";

        cache.store_document(&doc, Format::Html, content).unwrap();

        let retrieved = cache.get_document(&doc, Format::Html);
        assert_eq!(retrieved, Some(content.to_string()));
    }

    #[test]
    fn test_list_cached() {
        let (cache, _temp) = test_cache();

        cache
            .store_document(&DocumentType::Rfc(9000), Format::Html, "test")
            .unwrap();
        cache
            .store_document(&DocumentType::Rfc(8200), Format::Text, "test")
            .unwrap();

        let cached = cache.list_cached();
        assert_eq!(cached.len(), 2);
    }

    #[test]
    fn test_clear_cache() {
        let (cache, _temp) = test_cache();
        let doc = DocumentType::Rfc(9000);

        cache.store_document(&doc, Format::Html, "test").unwrap();
        assert!(cache.get_document(&doc, Format::Html).is_some());

        cache.clear_cache().unwrap();
        assert!(cache.get_document(&doc, Format::Html).is_none());
    }

    #[test]
    fn test_remove_document() {
        let (cache, _temp) = test_cache();
        let doc = DocumentType::Rfc(9000);

        // Remove non-existent returns false
        assert!(!cache.remove(&doc).unwrap());

        // Store both formats and metadata, then remove
        cache
            .store_document(&doc, Format::Html, "html content")
            .unwrap();
        cache
            .store_document(&doc, Format::Text, "text content")
            .unwrap();

        let meta = CacheMetadata {
            title: "Test Title".to_string(),
            cached_at: Utc::now(),
        };
        cache.store_metadata(&doc, &meta).unwrap();

        assert!(cache.remove(&doc).unwrap());

        // Verify both formats and metadata are gone
        assert!(cache.get_document(&doc, Format::Html).is_none());
        assert!(cache.get_document(&doc, Format::Text).is_none());
        assert!(cache.get_metadata(&doc).is_none());

        // Second remove returns false
        assert!(!cache.remove(&doc).unwrap());
    }

    #[test]
    fn test_remove_partial_formats() {
        let (cache, _temp) = test_cache();
        let doc = DocumentType::Rfc(8000);

        // Store only HTML
        cache
            .store_document(&doc, Format::Html, "html only")
            .unwrap();

        // Remove should succeed and return true
        assert!(cache.remove(&doc).unwrap());
        assert!(cache.get_document(&doc, Format::Html).is_none());
    }

    #[test]
    fn test_list_cached_with_drafts() {
        let (cache, _temp) = test_cache();

        let draft = DocumentType::Draft("draft-ietf-quic-transport-34".to_string());
        cache.store_document(&draft, Format::Text, "test").unwrap();

        let cached = cache.list_cached();
        assert_eq!(cached.len(), 1);
        assert!(cached.contains(&draft));
    }

    #[test]
    fn test_store_and_retrieve_metadata() {
        let (cache, _temp) = test_cache();
        let doc = DocumentType::Rfc(9000);
        let meta = CacheMetadata {
            title: "QUIC: A UDP-Based Multiplexed and Secure Transport".to_string(),
            cached_at: Utc::now(),
        };

        cache.store_metadata(&doc, &meta).unwrap();

        let retrieved = cache.get_metadata(&doc);
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.title, meta.title);
    }

    #[test]
    fn test_list_cached_with_metadata() {
        let (cache, _temp) = test_cache();

        let doc1 = DocumentType::Rfc(9000);
        let doc2 = DocumentType::Rfc(8200);

        cache
            .store_document(&doc1, Format::Text, "test")
            .unwrap();
        cache
            .store_document(&doc2, Format::Text, "test")
            .unwrap();

        let meta1 = CacheMetadata {
            title: "QUIC Transport".to_string(),
            cached_at: Utc::now(),
        };
        cache.store_metadata(&doc1, &meta1).unwrap();

        let cached = cache.list_cached_with_metadata();
        assert_eq!(cached.len(), 2);

        // doc1 has metadata
        let cached_doc1 = cached
            .iter()
            .find(|cd| cd.doc_type == doc1)
            .unwrap();
        assert!(cached_doc1.metadata.is_some());

        // doc2 doesn't have metadata
        let cached_doc2 = cached
            .iter()
            .find(|cd| cd.doc_type == doc2)
            .unwrap();
        assert!(cached_doc2.metadata.is_none());
    }

    #[test]
    fn test_metadata_missing() {
        let (cache, _temp) = test_cache();
        let doc = DocumentType::Rfc(9000);

        cache
            .store_document(&doc, Format::Text, "test")
            .unwrap();

        // Should return None for missing metadata
        assert!(cache.get_metadata(&doc).is_none());
    }
}
