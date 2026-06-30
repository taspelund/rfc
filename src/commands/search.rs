use anyhow::Result;

use crate::api::DataTrackerClient;
use crate::models::SearchFilter;

pub struct Args {
    pub query: String,
    pub filter: SearchFilter,
    pub limit: usize,
}

pub async fn run(args: Args) -> Result<()> {
    let client = DataTrackerClient::new()?;

    eprintln!("Searching for '{}'...", args.query);

    let results = client
        .search(&args.query, args.filter, args.limit as u32)
        .await?;

    if results.is_empty() {
        println!("No results found for '{}'", args.query);
        return Ok(());
    }

    let shown = results.len();

    let summary = match (results.total_count, results.has_more) {
        (Some(total), true) => format!("Showing {} of {} results", shown, total),
        (Some(total), false) => format!("Found {} results", total),
        (None, true) => format!("Showing {} results", shown),
        (None, false) => format!("Found {} results", shown),
    };
    let suffix = if results.has_more {
        ". Increase --limit <N> to show more"
    } else {
        ":"
    };
    println!("\n{}{}\n", summary, suffix);

    let max_name_width = results
        .documents
        .iter()
        .map(|doc| doc.doc_type.name().len())
        .max()
        .unwrap_or(10);

    // 80-col target line: name column + 2-space gutter + title.
    let title_width = 80_usize
        .saturating_sub(max_name_width)
        .saturating_sub(4)
        .min(77);

    for doc in &results.documents {
        println!(
            "{:<width$}  {}",
            doc.doc_type.name(),
            doc.short_title(title_width),
            width = max_name_width
        );
    }

    println!("\nUse 'rfc <document>' to read a document");
    Ok(())
}
