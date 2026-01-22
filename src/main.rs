use anyhow::{Context, Result};
use clap::Parser;
use std::env;
use std::io::Write;
use std::process::{Command, Stdio};

use rfc::{CacheManager, DataTrackerClient, DocumentFetcher, DocumentType, Format, SearchFilter};

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

    /// Use PAGER instead of EDITOR
    #[arg(short, long)]
    pager: bool,

    /// Program to open document with (overrides EDITOR/PAGER)
    #[arg(short = 'o', long, value_name = "PROGRAM")]
    open_with: Option<String>,

    /// Fetch fresh copy, ignoring cache
    #[arg(short, long)]
    fresh: bool,

    /// Open document in web browser (IETF Datatracker)
    #[arg(short = 'w', long, conflicts_with_all = ["pager", "open_with", "fresh"])]
    web: bool,

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
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle cache operations first
    if cli.list_cache {
        return list_cache();
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
        return search_documents(query, cli.limit.unwrap_or(100), filter).await;
    }

    // Default: view document
    if let Some(document) = &cli.document {
        return view_document(
            document,
            cli.pager,
            cli.open_with.as_deref(),
            cli.fresh,
            cli.web,
        )
        .await;
    }

    Ok(())
}

/// Parse document identifier into DocumentType
fn parse_document(doc: &str) -> Result<DocumentType> {
    // First try the standard parser
    if let Some(doc_type) = DocumentType::parse(doc) {
        return Ok(doc_type);
    }

    // If standard parsing failed, assume it's a draft name without the prefix
    let draft_name = if doc.starts_with("draft-") {
        doc.to_string()
    } else {
        format!("draft-{}", doc)
    };

    Ok(DocumentType::Draft(draft_name))
}

/// View a document using EDITOR or PAGER
async fn view_document(
    document: &str,
    use_pager: bool,
    open_with: Option<&str>,
    fresh: bool,
    web: bool,
) -> Result<()> {
    let doc_type = parse_document(document)?;

    // If web flag is set, open in browser instead
    if web {
        return open_in_browser(&doc_type);
    }

    let cache = CacheManager::new()?;
    let rfc_editor = DocumentFetcher::new()?;

    // Check cache first (unless fresh requested)
    let content = if !fresh {
        if let Some(cached) = cache.get_document(&doc_type, Format::Text) {
            eprintln!("Using cached copy of {}", doc_type);
            cached
        } else {
            fetch_and_cache(&doc_type, &cache, &rfc_editor).await?
        }
    } else {
        fetch_and_cache(&doc_type, &cache, &rfc_editor).await?
    };

    // Open in editor or pager
    open_in_viewer(&content, use_pager, open_with)?;

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

    Ok(text)
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

/// Open a document in the default web browser
fn open_in_browser(doc_type: &DocumentType) -> Result<()> {
    let url = doc_type.datatracker_url();
    eprintln!("Opening {} in browser...", doc_type);
    opener::open(&url).with_context(|| format!("Failed to open URL: {}", url))?;
    Ok(())
}

/// Open text in EDITOR or PAGER
fn open_in_viewer(text: &str, use_pager: bool, open_with: Option<&str>) -> Result<()> {
    let viewer = if let Some(program) = open_with {
        program.to_string()
    } else if use_pager {
        env::var("PAGER").unwrap_or_else(|_| "less".to_string())
    } else {
        env::var("EDITOR").unwrap_or_else(|_| "less".to_string())
    };

    // For editors, we need to write to a temp file
    // For pagers, we can pipe to stdin
    let is_pager = use_pager
        || viewer == "less"
        || viewer == "more"
        || viewer == "most"
        || viewer.contains("less")
        || viewer.contains("more");

    if is_pager {
        // Pipe to pager
        let mut child = Command::new(&viewer)
            .stdin(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to start pager: {}", viewer))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes())?;
        }

        child.wait()?;
    } else {
        // Write to temp file for editor
        let mut temp_file = tempfile::NamedTempFile::new()?;
        temp_file.write_all(text.as_bytes())?;
        temp_file.flush()?;

        let status = Command::new(&viewer)
            .arg(temp_file.path())
            .status()
            .with_context(|| format!("Failed to start editor: {}", viewer))?;

        if !status.success() {
            anyhow::bail!("Editor exited with non-zero status");
        }
    }

    Ok(())
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
    println!("\nFound {} results:\n", shown);

    for (i, doc) in results.documents.iter().enumerate() {
        println!("{}. {} - {}", i + 1, doc.doc_type, doc.title);
    }

    if results.has_more {
        println!("\n(More results available. Use -l to show more.)");
    }

    println!("\nUse 'rfc <document>' to read a document");

    Ok(())
}

/// List cached documents
fn list_cache() -> Result<()> {
    let cache = CacheManager::new()?;
    let cached = cache.list_cached();

    if cached.is_empty() {
        println!("Cache is empty");
    } else {
        println!("Cached documents ({}):\n", cached.len());
        for doc_type in cached {
            println!("  {}", doc_type);
        }
    }

    Ok(())
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

    // Calculate total size
    if let Ok(entries) = std::fs::read_dir(path) {
        let total_size: u64 = entries
            .filter_map(|e| e.ok())
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum();

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
    let doc_type = parse_document(document)?;

    if cache.remove(&doc_type)? {
        println!("Removed {} from cache", doc_type);
    } else {
        println!("{} was not in cache", doc_type);
    }

    Ok(())
}
