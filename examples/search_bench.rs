//! Benchmark four search-implementation prototypes against a fixed set of
//! queries with known-good ground-truth documents.
//!
//! Run with: `cargo run --release --example search_bench`
//!
//! Reports, per (prototype, query): wall-clock latency and whether the
//! ground-truth document appears in the result set.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

const BASE_URL: &str = "https://datatracker.ietf.org";
const USER_LIMIT: u32 = 25;

/// (query, ground-truth doc name that must appear in results)
const TEST_QUERIES: &[(&str, &str)] = &[
    ("bgp message", "rfc8654"),      // original bug case
    ("extended message", "rfc8654"), // word order swap
    ("bgp extended", "rfc4360"),
    ("quic", "rfc9000"), // single token, common
    ("json patch", "rfc6902"),
    ("tcp congestion", "rfc5681"),
    ("tls handshake", "rfc8446"),
    ("dns over https", "rfc8484"), // 3 tokens, "over" is noise
    ("http caching", "rfc9111"),
    ("bgp", "rfc4271"), // single token, very common
];

#[derive(Debug, Deserialize)]
struct SearchResponse {
    objects: Vec<ApiDoc>,
}

#[derive(Debug, Deserialize, Clone)]
struct ApiDoc {
    name: String,
    title: String,
    #[serde(rename = "abstract")]
    abstract_text: Option<String>,
}

fn tokenize(query: &str) -> Vec<String> {
    query.split_whitespace().map(str::to_lowercase).collect()
}

fn primary_token<'a>(tokens: &'a [String], fallback: &'a str) -> &'a str {
    tokens
        .iter()
        .max_by_key(|t| t.len())
        .map(String::as_str)
        .unwrap_or(fallback)
}

fn is_rfc_or_draft(name: &str) -> bool {
    name.starts_with("rfc") || name.starts_with("draft-")
}

/// Local AND filter: every `tokens` entry must appear in title or abstract.
fn matches_all_tokens(doc: &ApiDoc, tokens: &[&str]) -> bool {
    if tokens.is_empty() {
        return true;
    }
    let title_lc = doc.title.to_lowercase();
    let abs_lc = doc.abstract_text.as_deref().unwrap_or("").to_lowercase();
    tokens
        .iter()
        .all(|t| title_lc.contains(t) || abs_lc.contains(t))
}

async fn fetch(client: &Client, url: &str) -> Result<Vec<ApiDoc>> {
    let resp = client.get(url).send().await.context("send")?;
    if !resp.status().is_success() {
        anyhow::bail!("http {}", resp.status());
    }
    let body: SearchResponse = resp.json().await.context("parse")?;
    Ok(body.objects)
}

// -------- Prototype A: current production behavior --------
// Single request, no server-side type filter, api_limit = limit*5,
// local filter for rfc/draft + multi-token AND.
async fn proto_a(client: &Client, query: &str) -> Result<Vec<String>> {
    let tokens = tokenize(query);
    let primary = primary_token(&tokens, query);
    let api_limit = USER_LIMIT.saturating_mul(5);
    let url = format!(
        "{}/api/v1/doc/document/?title__icontains={}&limit={}&format=json",
        BASE_URL,
        urlencoding::encode(primary),
        api_limit
    );
    let docs = fetch(client, &url).await?;
    let extras: Vec<&str> = tokens
        .iter()
        .filter(|t| t.as_str() != primary)
        .map(String::as_str)
        .collect();
    Ok(docs
        .into_iter()
        .filter(|d| is_rfc_or_draft(&d.name))
        .filter(|d| matches_all_tokens(d, &extras))
        .take(USER_LIMIT as usize)
        .map(|d| d.name)
        .collect())
}

// -------- Prototype B: + server-side type__in=rfc,draft --------
async fn proto_b(client: &Client, query: &str) -> Result<Vec<String>> {
    let tokens = tokenize(query);
    let primary = primary_token(&tokens, query);
    let api_limit = USER_LIMIT.saturating_mul(5);
    let url = format!(
        "{}/api/v1/doc/document/?title__icontains={}&type__in=rfc,draft&limit={}&format=json",
        BASE_URL,
        urlencoding::encode(primary),
        api_limit
    );
    let docs = fetch(client, &url).await?;
    let extras: Vec<&str> = tokens
        .iter()
        .filter(|t| t.as_str() != primary)
        .map(String::as_str)
        .collect();
    Ok(docs
        .into_iter()
        .filter(|d| matches_all_tokens(d, &extras))
        .take(USER_LIMIT as usize)
        .map(|d| d.name)
        .collect())
}

// -------- Prototype C: parallel type=rfc + type=draft --------
async fn proto_c(client: &Client, query: &str) -> Result<Vec<String>> {
    let tokens = tokenize(query);
    let primary = primary_token(&tokens, query);
    let per_request_limit = USER_LIMIT.saturating_mul(5);

    let make_url = |t: &str| {
        format!(
            "{}/api/v1/doc/document/?title__icontains={}&type={}&limit={}&format=json",
            BASE_URL,
            urlencoding::encode(primary),
            t,
            per_request_limit
        )
    };

    let url_rfc = make_url("rfc");
    let url_draft = make_url("draft");
    let (rfcs, drafts) = tokio::join!(fetch(client, &url_rfc), fetch(client, &url_draft));
    let mut docs = rfcs?;
    docs.extend(drafts?);

    let extras: Vec<&str> = tokens
        .iter()
        .filter(|t| t.as_str() != primary)
        .map(String::as_str)
        .collect();
    Ok(docs
        .into_iter()
        .filter(|d| matches_all_tokens(d, &extras))
        .take(USER_LIMIT as usize)
        .map(|d| d.name)
        .collect())
}

// -------- Prototype D: server-side AND across title + abstract --------
// Push the longest token to title__icontains and the second-longest to
// abstract__icontains. Remaining tokens (if any) AND'd locally.
async fn proto_d(client: &Client, query: &str) -> Result<Vec<String>> {
    let tokens = tokenize(query);
    let primary = primary_token(&tokens, query);
    let secondary: Option<&str> = tokens
        .iter()
        .filter(|t| t.as_str() != primary)
        .max_by_key(|t| t.len())
        .map(String::as_str);

    let mut url = format!(
        "{}/api/v1/doc/document/?title__icontains={}&type__in=rfc,draft&limit={}&format=json",
        BASE_URL,
        urlencoding::encode(primary),
        USER_LIMIT
    );
    if let Some(s) = secondary {
        url.push_str(&format!("&abstract__icontains={}", urlencoding::encode(s)));
    }

    let docs = fetch(client, &url).await?;
    // Remaining tokens (3rd+) get filtered locally.
    let extras: Vec<&str> = tokens
        .iter()
        .filter(|t| t.as_str() != primary && Some(t.as_str()) != secondary)
        .map(String::as_str)
        .collect();
    Ok(docs
        .into_iter()
        .filter(|d| matches_all_tokens(d, &extras))
        .take(USER_LIMIT as usize)
        .map(|d| d.name)
        .collect())
}

struct RunResult {
    duration: Duration,
    found: bool,
    count: usize,
}

/// Run a prototype N+1 times: one warmup (used to assess recall) and N timed
/// runs whose latency is averaged.
macro_rules! run_one {
    ($client:expr, $query:expr, $truth:expr, $proto:ident, $runs:expr) => {{
        let warmup = $proto($client, $query).await?;
        let found = warmup.iter().any(|n| n == $truth);
        let count = warmup.len();
        let mut total = Duration::ZERO;
        for _ in 0..$runs {
            let start = Instant::now();
            let _ = $proto($client, $query).await?;
            total += start.elapsed();
        }
        RunResult {
            duration: total / $runs,
            found,
            count,
        }
    }};
}

#[tokio::main]
async fn main() -> Result<()> {
    let client = Client::builder()
        .user_agent("rfc-search-bench/0.1")
        .timeout(Duration::from_secs(30))
        .build()?;

    const RUNS: u32 = 3;

    println!(
        "{:<28} {:>10} {:>10} {:>10} {:>10}",
        "query / truth", "A (ms)", "B (ms)", "C (ms)", "D (ms)"
    );
    println!(
        "{:<28} {:>10} {:>10} {:>10} {:>10}",
        "                  recall", "A", "B", "C", "D"
    );
    println!("{}", "-".repeat(74));

    // Aggregate totals
    let mut totals = [Duration::ZERO; 4];
    let mut hits = [0u32; 4];

    for (q, truth) in TEST_QUERIES {
        let a = run_one!(&client, q, truth, proto_a, RUNS);
        let b = run_one!(&client, q, truth, proto_b, RUNS);
        let c = run_one!(&client, q, truth, proto_c, RUNS);
        let d = run_one!(&client, q, truth, proto_d, RUNS);

        totals[0] += a.duration;
        totals[1] += b.duration;
        totals[2] += c.duration;
        totals[3] += d.duration;
        if a.found {
            hits[0] += 1;
        }
        if b.found {
            hits[1] += 1;
        }
        if c.found {
            hits[2] += 1;
        }
        if d.found {
            hits[3] += 1;
        }

        let label = format!("{:<20} -> {}", q, truth);
        println!(
            "{:<28} {:>10.0} {:>10.0} {:>10.0} {:>10.0}",
            label,
            a.duration.as_millis(),
            b.duration.as_millis(),
            c.duration.as_millis(),
            d.duration.as_millis(),
        );
        println!(
            "{:<28} {:>10} {:>10} {:>10} {:>10}",
            format!(
                "  found? (n={})",
                a.count.max(b.count).max(c.count).max(d.count)
            ),
            mark(a.found),
            mark(b.found),
            mark(c.found),
            mark(d.found),
        );
    }

    println!("{}", "-".repeat(74));
    let n = TEST_QUERIES.len() as u32;
    println!(
        "{:<28} {:>10.0} {:>10.0} {:>10.0} {:>10.0}",
        format!("avg latency (ms, n={})", n),
        (totals[0] / n).as_millis(),
        (totals[1] / n).as_millis(),
        (totals[2] / n).as_millis(),
        (totals[3] / n).as_millis(),
    );
    println!(
        "{:<28} {:>10} {:>10} {:>10} {:>10}",
        format!("recall ({}/{})", "x", n),
        format!("{}/{}", hits[0], n),
        format!("{}/{}", hits[1], n),
        format!("{}/{}", hits[2], n),
        format!("{}/{}", hits[3], n),
    );

    Ok(())
}

fn mark(found: bool) -> &'static str {
    if found {
        "yes"
    } else {
        "MISS"
    }
}
