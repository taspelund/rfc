use chrono::Utc;
use rfc::{CacheManager, CacheMetadata, DocumentType, Format};
use tempfile::TempDir;

#[test]
fn test_metadata_integration() {
    let temp_dir = TempDir::new().unwrap();
    let cache = CacheManager::with_dir(temp_dir.path().to_path_buf()).unwrap();

    // Create test documents
    let doc1 = DocumentType::Rfc(4271);
    let doc2 = DocumentType::Rfc(9000);
    let doc3 = DocumentType::Rfc(8200);

    // Store documents
    cache
        .store_document(&doc1, Format::Text, "BGP content")
        .unwrap();
    cache
        .store_document(&doc2, Format::Text, "QUIC content")
        .unwrap();
    cache
        .store_document(&doc3, Format::Text, "IPv6 content")
        .unwrap();

    // Store metadata for doc1 and doc2 only
    let meta1 = CacheMetadata {
        title: "A Border Gateway Protocol 4 (BGP-4)".to_string(),
        cached_at: Utc::now(),
    };
    cache.store_metadata(&doc1, &meta1).unwrap();

    let meta2 = CacheMetadata {
        title: "QUIC: A UDP-Based Multiplexed and Secure Transport".to_string(),
        cached_at: Utc::now(),
    };
    cache.store_metadata(&doc2, &meta2).unwrap();

    // List with metadata
    let cached = cache.list_cached_with_metadata();

    // Should have all 3 documents
    assert_eq!(cached.len(), 3);

    // Find each document and verify metadata
    let cached_doc1 = cached.iter().find(|cd| cd.doc_type == doc1).unwrap();
    assert!(cached_doc1.metadata.is_some());
    assert_eq!(cached_doc1.metadata.as_ref().unwrap().title, meta1.title);

    let cached_doc2 = cached.iter().find(|cd| cd.doc_type == doc2).unwrap();
    assert!(cached_doc2.metadata.is_some());
    assert_eq!(cached_doc2.metadata.as_ref().unwrap().title, meta2.title);

    let cached_doc3 = cached.iter().find(|cd| cd.doc_type == doc3).unwrap();
    assert!(cached_doc3.metadata.is_none());
}

#[test]
fn test_title_truncation() {
    let long_title = "This is a very long title that should be truncated when displayed in list cache";

    // Test truncation with different widths
    let truncated = truncate_title(long_title, 60);
    // Should be 60 - 3 = 57 characters from title + 3 for "..."
    assert_eq!(truncated.len(), 60); // Total should be 60
    assert!(truncated.ends_with("..."));

    // Test no truncation with wide mode
    let not_truncated = truncate_title(long_title, usize::MAX);
    assert_eq!(not_truncated, long_title);

    // Test UTF-8 handling
    let utf8_title = "QUIC: A UDP-Based Multiplexed and Secure Transport";
    let truncated_utf8 = truncate_title(utf8_title, 30);
    assert!(truncated_utf8.ends_with("..."));
    assert!(truncated_utf8.len() <= 33); // 30 + 3
}

fn truncate_title(title: &str, max_width: usize) -> String {
    if max_width == usize::MAX || title.chars().count() <= max_width {
        title.to_string()
    } else {
        let truncated: String = title
            .chars()
            .take(max_width.saturating_sub(3))
            .collect();
        format!("{}...", truncated)
    }
}
