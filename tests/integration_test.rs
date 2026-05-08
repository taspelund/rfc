use chrono::Utc;
use rfc::{CacheManager, CacheMetadata, DocumentType, Format};
use tempfile::TempDir;

#[test]
fn cache_listing_pairs_documents_with_metadata() {
    let temp_dir = TempDir::new().unwrap();
    let cache = CacheManager::with_dir(temp_dir.path().to_path_buf()).unwrap();

    let with_meta = DocumentType::Rfc(4271);
    let with_meta_2 = DocumentType::Rfc(9000);
    let without_meta = DocumentType::Rfc(8200);

    for doc in [&with_meta, &with_meta_2, &without_meta] {
        cache.store_document(doc, Format::Text, "content").unwrap();
    }

    let title_1 = "A Border Gateway Protocol 4 (BGP-4)".to_string();
    let title_2 = "QUIC: A UDP-Based Multiplexed and Secure Transport".to_string();
    cache
        .store_metadata(
            &with_meta,
            &CacheMetadata {
                title: title_1.clone(),
                cached_at: Utc::now(),
            },
        )
        .unwrap();
    cache
        .store_metadata(
            &with_meta_2,
            &CacheMetadata {
                title: title_2.clone(),
                cached_at: Utc::now(),
            },
        )
        .unwrap();

    let cached = cache.list_cached_with_metadata();
    assert_eq!(cached.len(), 3);

    let lookup = |dt: &DocumentType| {
        cached
            .iter()
            .find(|cd| &cd.doc_type == dt)
            .expect("entry present")
    };

    assert_eq!(lookup(&with_meta).metadata.as_ref().unwrap().title, title_1);
    assert_eq!(
        lookup(&with_meta_2).metadata.as_ref().unwrap().title,
        title_2
    );
    assert!(lookup(&without_meta).metadata.is_none());
}
