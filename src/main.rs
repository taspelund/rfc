use anyhow::{Context, Result};
use clap::Parser;
use std::env;
use std::io::Write;
use std::process::Command;

use rfc::{
    CacheManager, CacheMetadata, DataTrackerClient, DocumentFetcher, DocumentType, Format,
    SearchFilter,
};

#[derive(Parser)]
#[command(name = "rfc")]
#[command(about = "Search, retrieve, and display IETF RFCs and drafts")]
#[command(version)]
#[command(arg_required_else_help = true)]
struct Cli {
    /// RFC number or draft name to view
    document: Option<String>,

    /// Search for documents
    #[arg(short, long, value_name = "QUERY")]
    search: Option<String>,

    /// Program to open document with
    #[arg(short = 'o', long, value_name = "PROGRAM")]
    open_with: Option<String>,

    /// Fetch the document, but do not open it (implies -r)
    #[arg(short = 'f', long)]
    fetch_only: bool,

    /// Fetch from API and refresh cache before opening
    #[arg(short = 'r', long)]
    refresh: bool,

    /// Only show drafts (with -s)
    #[arg(short, long, conflicts_with = "all")]
    drafts: bool,

    /// Show both RFCs and drafts (with -s)
    #[arg(short, long, conflicts_with = "drafts")]
    all: bool,

    /// Limit search results (with -s)
    #[arg(short, long)]
    limit: Option<usize>,

    /// List cached documents
    #[arg(long)]
    list_cache: bool,

    /// Clear all cached documents
    #[arg(long)]
    clear_cache: bool,

    /// Show cache info
    #[arg(long)]
    cache_info: bool,

    /// Remove a document from cache
    #[arg(long, value_name = "DOC")]
    uncache: Option<String>,

    /// Show full titles without truncation (with --list-cache)
    #[arg(short = 'w', long)]
    wide: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle cache operations first
    if cli.list_cache {
        return list_cache(cli.wide);
    }
    if cli.clear_cache {
        return clear_cache();
    }
    if cli.cache_info {
        return cache_info();
    }
    if let Some(doc) = &cli.uncache {
        return uncache_document(doc);
    }

    // Handle search
    if let Some(query) = &cli.search {
        let filter = if cli.drafts {
            SearchFilter::DraftsOnly
        } else if cli.all {
            SearchFilter::Both
        } else {
            SearchFilter::RfcsOnly
        };
        return search_documents(query, cli.limit.unwrap_or(25), filter).await;
    }

    // Default: view document
    if let Some(document) = &cli.document {
        // -f/--fetch-only: fetch from API, cache, and don't open
        // (implies refreshing from API)
        if cli.fetch_only {
            return fetch_only_document(document).await;
        }
        // Otherwise: view with optional refresh
        // -r/--refresh: fetch from API before opening
        return view_document(document, cli.open_with.as_deref(), cli.refresh).await;
    }

    Ok(())
}

/// View a document, optionally refreshing from API
async fn view_document(document: &str, open_with: Option<&str>, refresh: bool) -> Result<()> {
    let doc_type = DocumentType::from_user_input(document);
    let cache = CacheManager::new()?;
    let rfc_editor = DocumentFetcher::new()?;

    // Check cache first (unless refresh requested)
    let content = if !refresh {
        if let Some(cached) = cache.get_document(&doc_type, Format::Text) {
            eprintln!("Using cached copy of {}", doc_type);
            cached
        } else {
            fetch_and_cache(&doc_type, &cache, &rfc_editor).await?
        }
    } else {
        fetch_and_cache(&doc_type, &cache, &rfc_editor).await?
    };

    // Open in viewer if a program is specified or defaults are available
    open_in_viewer(&content, open_with)?;

    Ok(())
}

/// Fetch a document and cache it without opening
async fn fetch_only_document(document: &str) -> Result<()> {
    let doc_type = DocumentType::from_user_input(document);
    let cache = CacheManager::new()?;
    let rfc_editor = DocumentFetcher::new()?;

    eprintln!("Fetching {}...", doc_type);
    fetch_and_cache(&doc_type, &cache, &rfc_editor).await?;
    eprintln!("Cached {}. Use 'rfc {}' to view.", doc_type, doc_type);

    Ok(())
}

/// Fetch document and store in cache
async fn fetch_and_cache(
    doc_type: &DocumentType,
    cache: &CacheManager,
    rfc_editor: &DocumentFetcher,
) -> Result<String> {
    eprintln!("Fetching {}...", doc_type);

    // Try text first, fall back to HTML
    let (content, format) = rfc_editor.fetch(doc_type).await?;

    // Convert HTML to text if needed
    let text = match format {
        Format::Text => content,
        Format::Html => {
            eprintln!("Plain text not available, converting from HTML...");
            html_to_text(&content)
        }
    };

    // Cache the text content
    cache.store_document(doc_type, Format::Text, &text)?;

    // Fetch and cache metadata (best effort - don't fail if this fails)
    if let Err(e) = fetch_and_store_metadata(doc_type, cache).await {
        eprintln!("Warning: Failed to fetch metadata for {}: {}", doc_type, e);
    }

    Ok(text)
}

/// Fetch metadata from Datatracker and store it
async fn fetch_and_store_metadata(doc_type: &DocumentType, cache: &CacheManager) -> Result<()> {
    let client = DataTrackerClient::new()?;
    let doc = client.get_document(&doc_type.name()).await?;

    let metadata = CacheMetadata {
        title: doc.title,
        cached_at: chrono::Utc::now(),
    };

    cache.store_metadata(doc_type, &metadata)?;
    Ok(())
}

/// Convert HTML to plain text
fn html_to_text(html: &str) -> String {
    html2text::from_read(html.as_bytes(), 80).unwrap_or_else(|e| {
        eprintln!(
            "Warning: HTML to text conversion failed ({}), displaying raw HTML",
            e
        );
        html.to_string()
    })
}

/// Open text in EDITOR or PAGER
/// Open text in a viewer
///
/// Default behavior (no --open-with):
/// 1. Try $EDITOR environment variable
/// 2. Fall back to $PAGER environment variable
/// 3. If neither is set, don't open (just return)
///
/// With --open-with <PROGRAM>: use that specific program
fn open_in_viewer(text: &str, open_with: Option<&str>) -> Result<()> {
    let viewer_str = match open_with {
        Some(program) => program.to_string(),
        None => {
            if let Ok(editor) = env::var("EDITOR") {
                editor
            } else if let Ok(pager) = env::var("PAGER") {
                pager
            } else {
                return Ok(());
            }
        }
    };

    let (program, extra_args) = split_viewer_command(&viewer_str)
        .with_context(|| format!("Empty viewer command: {:?}", viewer_str))?;

    let mut temp_file = tempfile::NamedTempFile::new()?;
    temp_file.write_all(text.as_bytes())?;
    temp_file.flush()?;

    let status = Command::new(&program)
        .args(&extra_args)
        .arg(temp_file.path())
        .status()
        .with_context(|| format!("Failed to start viewer: {}", program))?;

    if !status.success() {
        anyhow::bail!("Viewer exited with non-zero status");
    }

    Ok(())
}

/// Split a viewer command string into `(program, args)` on whitespace.
/// Returns `None` if the string is empty/whitespace-only.
fn split_viewer_command(s: &str) -> Option<(String, Vec<String>)> {
    let mut parts = s.split_whitespace().map(String::from);
    let program = parts.next()?;
    Some((program, parts.collect()))
}

/// Search for documents
async fn search_documents(query: &str, limit: usize, filter: SearchFilter) -> Result<()> {
    let client = DataTrackerClient::new()?;

    eprintln!("Searching for '{}'...", query);

    let results = client.search(query, filter, limit as u32).await?;

    if results.is_empty() {
        println!("No results found for '{}'", query);
        return Ok(());
    }

    let shown = results.len();

    // Report results with total count if available
    if let Some(total) = results.total_count {
        if results.has_more {
            println!(
                "\nShowing {} of {} results. Increase --limit <N> to show more.\n",
                shown, total
            );
        } else {
            println!("\nFound {} results:\n", total);
        }
    } else {
        // Fallback if total count not available
        if results.has_more {
            println!(
                "\nShowing {} results. Increase --limit <N> to show more.\n",
                shown
            );
        } else {
            println!("\nFound {} results:\n", shown);
        }
    }

    // Calculate max name width for alignment (80 char total line width)
    let max_name_width = results
        .documents
        .iter()
        .map(|doc| doc.doc_type.name().len())
        .max()
        .unwrap_or(10);

    // Available width for title: 80 total - name - 2 separator - some margin
    let available_width = 80_usize.saturating_sub(max_name_width).saturating_sub(4);
    let title_width = available_width.min(77); // Reasonable min

    for doc in &results.documents {
        let name = doc.doc_type.name();
        let title = truncate_title(&doc.title, title_width);
        println!("{:<width$}  {}", name, title, width = max_name_width);
    }

    println!("\nUse 'rfc <document>' to read a document");

    Ok(())
}

/// List cached documents with optional titles
fn list_cache(wide: bool) -> Result<()> {
    let cache = CacheManager::new()?;
    let cached = cache.list_cached_with_metadata();

    if cached.is_empty() {
        println!("Cache is empty");
        return Ok(());
    }

    println!("Cached documents ({}):\n", cached.len());

    // Calculate max name width for alignment (80 char total line width)
    let max_name_width = cached
        .iter()
        .map(|cd| cd.doc_type.name().len())
        .max()
        .unwrap_or(10);

    // Available width for title: 80 total - name - 2 separator - some margin
    let available_width = 80_usize.saturating_sub(max_name_width).saturating_sub(4);
    let title_width = if wide {
        usize::MAX
    } else {
        available_width.min(77)
    };
    let mut missing_count = 0;

    for cached_doc in &cached {
        let name = cached_doc.doc_type.name();

        if let Some(meta) = &cached_doc.metadata {
            let title = truncate_title(&meta.title, title_width);
            println!("{:<width$}  {}", name, title, width = max_name_width);
        } else {
            println!(
                "{:<width$}  (title unavailable)",
                name,
                width = max_name_width
            );
            missing_count += 1;
        }
    }

    if missing_count > 0 {
        println!(
            "\n({} document{} without title - re-cache with --fresh to fetch metadata)",
            missing_count,
            if missing_count == 1 { "" } else { "s" }
        );
    }

    Ok(())
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

/// Truncate title to a maximum width
fn truncate_title(title: &str, max_width: usize) -> String {
    if max_width == usize::MAX || title.chars().count() <= max_width {
        title.to_string()
    } else {
        let truncated: String = title.chars().take(max_width.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}

/// Clear all cached documents
fn clear_cache() -> Result<()> {
    let cache = CacheManager::new()?;
    cache.clear_cache()?;
    println!("Cache cleared");
    Ok(())
}

/// Show cache info
fn cache_info() -> Result<()> {
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

/// Remove a document from cache
fn uncache_document(document: &str) -> Result<()> {
    let cache = CacheManager::new()?;
    let doc_type = DocumentType::from_user_input(document);

    if cache.remove(&doc_type)? {
        println!("Removed {} from cache", doc_type);
    } else {
        println!("{} was not in cache", doc_type);
    }

    Ok(())
}
