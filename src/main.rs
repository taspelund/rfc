use anyhow::Result;
use clap::{Args, Parser, Subcommand};

use rfc::commands;
use rfc::SearchFilter;

#[derive(Parser)]
#[command(name = "rfc", version)]
#[command(about = "Search, retrieve, and display IETF RFCs and drafts")]
#[command(args_conflicts_with_subcommands = true)]
#[command(arg_required_else_help = true)]
struct Cli {
    /// RFC number or draft name to view (default action — uses cache when present)
    document: Option<String>,

    /// Program to open the document with (defaults to $EDITOR, then $PAGER)
    #[arg(short = 'o', long, value_name = "PROGRAM")]
    open_with: Option<String>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Fetch a document from the API and cache it without opening it
    Fetch { document: String },

    /// Search the IETF Datatracker
    Search(SearchArgs),

    /// Manage the local document cache
    #[command(subcommand)]
    Cache(CacheCmd),
}

#[derive(Args)]
struct SearchArgs {
    /// Query string (whitespace-separated tokens are AND-ed together)
    #[arg(required = true, num_args = 1..)]
    query: Vec<String>,

    #[command(flatten)]
    filter: SearchFilterArgs,

    /// Maximum number of results to display
    #[arg(short, long, default_value_t = 25)]
    limit: usize,
}

#[derive(Args)]
#[group(multiple = false)]
struct SearchFilterArgs {
    /// Only show Internet-Drafts
    #[arg(short, long)]
    drafts: bool,

    /// Show both RFCs and drafts
    #[arg(short, long)]
    all: bool,
}

impl From<&SearchFilterArgs> for SearchFilter {
    fn from(a: &SearchFilterArgs) -> Self {
        if a.drafts {
            SearchFilter::DraftsOnly
        } else if a.all {
            SearchFilter::Both
        } else {
            SearchFilter::RfcsOnly
        }
    }
}

#[derive(Subcommand)]
enum CacheCmd {
    /// List cached documents
    List {
        /// Show full titles without truncation
        #[arg(short, long)]
        wide: bool,
    },
    /// Show cache location and total size
    Info,
    /// Remove a single document from the cache
    Remove { document: String },
    /// Remove every cached document
    Clear,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Fetch { document }) => commands::fetch::run(&document).await,
        Some(Command::Search(args)) => {
            let filter = SearchFilter::from(&args.filter);
            commands::search::run(commands::search::Args {
                query: args.query.join(" "),
                filter,
                limit: args.limit,
            })
            .await
        }
        Some(Command::Cache(c)) => match c {
            CacheCmd::List { wide } => commands::cache::list(wide),
            CacheCmd::Info => commands::cache::info(),
            CacheCmd::Remove { document } => commands::cache::remove(&document),
            CacheCmd::Clear => commands::cache::clear(),
        },
        None => match cli.document {
            Some(doc) => commands::view::run(&doc, cli.open_with.as_deref()).await,
            // arg_required_else_help handles the "no args at all" case.
            None => Ok(()),
        },
    }
}
