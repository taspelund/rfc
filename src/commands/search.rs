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

    println!("\n{}\n", format_search_summary(shown, results.total_count, results.has_more));

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

/// Format the search results summary line (shown count, total count, "show more" hint).
pub(crate) fn format_search_summary(shown: usize, total_count: Option<u32>, has_more: bool) -> String {
    let summary = match (total_count, has_more) {
        (Some(total), true) => format!("Showing {} of {} results", shown, total),
        (Some(total), false) => format!("Found {} results", total),
        (None, true) => format!("Showing {} results", shown),
        (None, false) => format!("Found {} results", shown),
    };
    let suffix = if has_more {
        ". Increase --limit <N> to show more"
    } else {
        ":"
    };
    format!("{}{}", summary, suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_with_total_and_more() {
        assert_eq!(
            format_search_summary(10, Some(25), true),
            "Showing 10 of 25 results. Increase --limit <N> to show more"
        );
    }

    #[test]
    fn summary_with_total_and_done() {
        assert_eq!(
            format_search_summary(25, Some(25), false),
            "Found 25 results:"
        );
    }

    #[test]
    fn summary_no_total_with_more() {
        assert_eq!(
            format_search_summary(5, None, true),
            "Showing 5 results. Increase --limit <N> to show more"
        );
    }

    #[test]
    fn summary_no_total_and_done() {
        assert_eq!(
            format_search_summary(5, None, false),
            "Found 5 results:"
        );
    }

    #[test]
    fn summary_zero_results() {
        assert_eq!(
            format_search_summary(0, Some(0), false),
            "Found 0 results:"
        );
    }
}
