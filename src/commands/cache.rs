use anyhow::Result;

use crate::cache::CacheManager;
use crate::models::DocumentType;

pub fn list(wide: bool) -> Result<()> {
    let cache = CacheManager::new()?;
    let cached = cache.list_cached_with_metadata();

    if cached.is_empty() {
        println!("Cache is empty");
        return Ok(());
    }

    println!("Cached documents ({}):\n", cached.len());

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
    let mut missing_count = 0;

    for cached_doc in &cached {
        let name = cached_doc.doc_type.name();
        match &cached_doc.metadata {
            Some(meta) => {
                let title = truncate(&meta.title, title_width);
                println!("{:<width$}  {}", name, title, width = max_name_width);
            }
            None => {
                println!(
                    "{:<width$}  (title unavailable)",
                    name,
                    width = max_name_width
                );
                missing_count += 1;
            }
        }
    }

    if missing_count > 0 {
        println!(
            "\n({} document{} without title - run 'rfc fetch <doc>' to refresh metadata)",
            missing_count,
            if missing_count == 1 { "" } else { "s" }
        );
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
        let size_str = if total_size < 1024 {
            format!("{} B", total_size)
        } else if total_size < 1024 * 1024 {
            format!("{:.1} KB", total_size as f64 / 1024.0)
        } else {
            format!("{:.1} MB", total_size as f64 / (1024.0 * 1024.0))
        };
        println!("Total size: {}", size_str);
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

/// Truncate `s` to `max_width` characters, replacing the tail with `...`
/// when the string would be longer. Counts unicode scalar values, not bytes.
fn truncate(s: &str, max_width: usize) -> String {
    if max_width == usize::MAX || s.chars().count() <= max_width {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_width.saturating_sub(3)).collect();
        format!("{}...", truncated)
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
    fn truncate_short_string() {
        assert_eq!(truncate("hello", 80), "hello");
    }

    #[test]
    fn truncate_long_string() {
        assert_eq!(truncate("hello world", 8), "hello...");
    }

    #[test]
    fn truncate_unlimited() {
        assert_eq!(truncate("hello world", usize::MAX), "hello world");
    }
}
