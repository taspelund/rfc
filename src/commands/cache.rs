use anyhow::Result;

use crate::cache::{CacheManager, CachedDocument};
use crate::models::DocumentType;
use crate::util::truncate_str;

pub fn list(wide: bool) -> Result<()> {
    let cache = CacheManager::new()?;
    let cached = cache.list_cached_with_metadata();

    if cached.is_empty() {
        println!("Cache is empty");
        return Ok(());
    }

    println!("Cached documents ({}):\n", cached.len());
    for line in format_cache_list(&cached, wide) {
        println!("{}", line);
    }

    Ok(())
}

pub fn info() -> Result<()> {
    let cache = CacheManager::new()?;
    let path = cache.cache_dir();
    let cached = cache.list_cached();

    println!("Cache directory: {}", path.display());
    println!("Cached documents: {}", cached.len());

    if let Ok(total_size) = dir_size_recursive(path) {
        println!("Total size: {}", format_cache_size(total_size));
    }

    Ok(())
}

pub fn clear() -> Result<()> {
    let cache = CacheManager::new()?;
    cache.clear_cache()?;
    println!("Cache cleared");
    Ok(())
}

pub fn remove(document: &str) -> Result<()> {
    let cache = CacheManager::new()?;
    let doc_type = DocumentType::from_user_input(document);

    if cache.remove(&doc_type)? {
        println!("Removed {} from cache", doc_type);
    } else {
        println!("{} was not in cache", doc_type);
    }
    Ok(())
}

pub(crate) fn format_cache_list(cached: &[CachedDocument], wide: bool) -> Vec<String> {
    if cached.is_empty() {
        return vec!["Cache is empty".to_string()];
    }

    let max_name_width = cached
        .iter()
        .map(|cd| cd.doc_type.name().len())
        .max()
        .unwrap_or(10);

    let title_width = if wide {
        usize::MAX
    } else {
        80_usize
            .saturating_sub(max_name_width)
            .saturating_sub(4)
            .min(77)
    };

    let mut lines = Vec::new();
    let mut missing_count = 0;

    for cached_doc in cached {
        let name = cached_doc.doc_type.name();
        match &cached_doc.metadata {
            Some(meta) => {
                let title = truncate_str(&meta.title, title_width);
                lines.push(format!(
                    "{:<width$}  {}",
                    name,
                    title,
                    width = max_name_width
                ));
            }
            None => {
                lines.push(format!(
                    "{:<width$}  (title unavailable)",
                    name,
                    width = max_name_width
                ));
                missing_count += 1;
            }
        }
    }

    if missing_count > 0 {
        lines.push(format!(
            "\n({} document{} without title - run 'rfc fetch <doc>' to refresh metadata)",
            missing_count,
            if missing_count == 1 { "" } else { "s" }
        ));
    }

    lines
}

pub(crate) fn format_cache_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Sum the sizes of all regular files under `dir`, recursively.
fn dir_size_recursive(dir: &std::path::Path) -> std::io::Result<u64> {
    let mut total = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            if ft.is_dir() {
                stack.push(entry.path());
            } else if ft.is_file() {
                total += entry.metadata()?.len();
            }
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::CacheMetadata;
    use chrono::Utc;
    use tempfile::TempDir;

    #[test]
    fn dir_size_recursive_sums_nested_files() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("documents");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("a.txt"), "hello").unwrap(); // 5
        std::fs::write(nested.join("b.txt"), "world!!").unwrap(); // 7
        std::fs::write(dir.path().join("top.txt"), "x").unwrap(); // 1

        assert_eq!(dir_size_recursive(dir.path()).unwrap(), 13);
    }

    #[test]
    fn format_cache_list_empty() {
        let lines = format_cache_list(&[], false);
        assert_eq!(lines, vec!["Cache is empty"]);
    }

    #[test]
    fn format_cache_list_with_metadata() {
        let meta = CacheMetadata {
            title: "BGP-4".to_string(),
            cached_at: Utc::now(),
        };
        let cached = vec![CachedDocument {
            doc_type: DocumentType::Rfc(4271),
            metadata: Some(meta),
        }];
        let lines = format_cache_list(&cached, false);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("rfc4271"));
        assert!(lines[0].contains("BGP-4"));
    }

    #[test]
    fn format_cache_list_missing_metadata() {
        let cached = vec![CachedDocument {
            doc_type: DocumentType::Rfc(8200),
            metadata: None,
        }];
        let lines = format_cache_list(&cached, false);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("(title unavailable)"));
        assert!(lines[1].contains("1 document without title"));
    }

    #[test]
    fn format_cache_list_mixed_metadata() {
        let meta = CacheMetadata {
            title: "QUIC".to_string(),
            cached_at: Utc::now(),
        };
        let cached = vec![
            CachedDocument {
                doc_type: DocumentType::Rfc(9000),
                metadata: Some(meta),
            },
            CachedDocument {
                doc_type: DocumentType::Rfc(8200),
                metadata: None,
            },
        ];
        let lines = format_cache_list(&cached, false);
        assert_eq!(lines.len(), 3);
        assert!(lines[2].contains("1 document without title"));
    }

    #[test]
    fn format_cache_size_bytes() {
        assert_eq!(format_cache_size(0), "0 B");
        assert_eq!(format_cache_size(512), "512 B");
        assert_eq!(format_cache_size(1023), "1023 B");
    }

    #[test]
    fn format_cache_size_kb() {
        assert_eq!(format_cache_size(1024), "1.0 KB");
        assert_eq!(format_cache_size(1536), "1.5 KB");
    }

    #[test]
    fn format_cache_size_mb() {
        assert_eq!(format_cache_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_cache_size(2_000_000), "1.9 MB");
    }
}
