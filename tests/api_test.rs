use rfc::{DataTrackerClient, DocumentFetcher, DocumentType, Format, SearchFilter};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Build a shared HTTP client for tests.
fn test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap()
}

// ─── DataTrackerClient::search ────────────────────────────────────────

#[tokio::test]
async fn search_single_token_returns_documents() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/doc/document/"))
        .and(query_param("title__icontains", "quic"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                r#"{
                    "meta": { "total_count": 2, "next": null },
                    "objects": [
                        {"name": "rfc9000", "title": "QUIC", "abstract": "Transport"},
                        {"name": "rfc8999", "title": "QUIC Invariant", "abstract": "Invariants"}
                    ]
                }"#,
                "application/json",
            ),
        )
        .mount(&server)
        .await;

    let client = DataTrackerClient::with_client_and_base_url(test_client(), server.uri());
    let result = client.search("quic", SearchFilter::Both, 25).await.unwrap();

    assert_eq!(result.len(), 2);
    assert!(!result.has_more);
    assert_eq!(result.total_count, Some(2));
}

#[tokio::test]
async fn search_http_500_returns_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/doc/document/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let client = DataTrackerClient::with_client_and_base_url(test_client(), server.uri());
    let result = client.search("quic", SearchFilter::RfcsOnly, 25).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn search_empty_response_returns_empty() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/doc/document/"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                r#"{"meta": {"total_count": 0, "next": null}, "objects": []}"#,
                "application/json",
            ),
        )
        .mount(&server)
        .await;

    let client = DataTrackerClient::with_client_and_base_url(test_client(), server.uri());
    let result = client.search("quic", SearchFilter::RfcsOnly, 25).await.unwrap();

    assert!(result.is_empty());
}

#[tokio::test]
async fn search_type_filter_rfc_sends_type_param() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/doc/document/"))
        .and(query_param("type__in", "rfc"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                r#"{"meta": {}, "objects": [{"name": "rfc9000", "title": "QUIC"}]}"#,
                "application/json",
            ),
        )
        .mount(&server)
        .await;

    let client = DataTrackerClient::with_client_and_base_url(test_client(), server.uri());
    client.search("quic", SearchFilter::RfcsOnly, 25).await.unwrap();
}

#[tokio::test]
async fn search_type_filter_draft_sends_type_param() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/doc/document/"))
        .and(query_param("type__in", "draft"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                r#"{"meta": {}, "objects": [{"name": "draft-ietf-quic-00", "title": "QUIC"}]}"#,
                "application/json",
            ),
        )
        .mount(&server)
        .await;

    let client = DataTrackerClient::with_client_and_base_url(test_client(), server.uri());
    client.search("quic", SearchFilter::DraftsOnly, 25).await.unwrap();
}

#[tokio::test]
async fn search_extra_tokens_filter_locally() {
    let server = MockServer::start().await;
    // Three tokens "bgp message format" → primary="message", secondary="format",
    // extra=["bgp"]
    Mock::given(method("GET"))
        .and(path("/api/v1/doc/document/"))
        .and(query_param("title__icontains", "message"))
        .and(query_param("abstract__icontains", "format"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                r#"{
                    "meta": {},
                    "objects": [
                        {"name": "rfc4271", "title": "BGP-4", "abstract": "A message format"},
                        {"name": "rfc1000", "title": "Something else", "abstract": "No match here"},
                        {"name": "rfc2000", "title": "Also no", "abstract": "Nothing"}
                    ]
                }"#,
                "application/json",
            ),
        )
        .mount(&server)
        .await;

    let client = DataTrackerClient::with_client_and_base_url(test_client(), server.uri());
    let result = client.search("bgp message format", SearchFilter::RfcsOnly, 25)
        .await
        .unwrap();

    // Only rfc4271 matches "bgp" in title or abstract
    assert_eq!(result.len(), 1);
    assert_eq!(result.documents[0].name, "rfc4271");
    // total_count is None because extra tokens required local filtering
    assert_eq!(result.total_count, None);
}

// ─── DataTrackerClient::get_document ──────────────────────────────────

#[tokio::test]
async fn get_document_found_returns_document() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/doc/document/rfc9000/"))
        .and(query_param("format", "json"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                r#"{"name": "rfc9000", "title": "QUIC: A UDP-Based Multiplexed and Secure Transport"}"#,
                "application/json",
            ),
        )
        .mount(&server)
        .await;

    let client = DataTrackerClient::with_client_and_base_url(test_client(), server.uri());
    let doc = client.get_document("rfc9000").await.unwrap();

    assert_eq!(doc.name, "rfc9000");
    assert!(doc.title.contains("QUIC"));
}

#[tokio::test]
async fn get_document_not_found_returns_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/doc/document/rfc999999/"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let client = DataTrackerClient::with_client_and_base_url(test_client(), server.uri());
    let result = client.get_document("rfc999999").await;

    assert!(result.is_err());
}

// ─── DocumentFetcher::fetch (RFC, text success) ───────────────────────

#[tokio::test]
async fn fetch_rfc_text_success() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rfc/rfc9000.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_string("RFC 9000 content"))
        .mount(&server)
        .await;

    let fetcher = DocumentFetcher::with_client_and_urls(
        test_client(),
        &server.uri(),
        &server.uri(),
        &server.uri(),
    );
    let (content, format) = fetcher.fetch(&DocumentType::Rfc(9000)).await.unwrap();

    assert_eq!(content, "RFC 9000 content");
    assert_eq!(format, Format::Text);
}

#[tokio::test]
async fn fetch_rfc_text_404_fallback_html() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rfc/rfc9000.txt"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rfc/rfc9000.html"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>RFC 9000 HTML</html>"))
        .mount(&server)
        .await;

    let fetcher = DocumentFetcher::with_client_and_urls(
        test_client(),
        &server.uri(),
        &server.uri(),
        &server.uri(),
    );
    let (content, format) = fetcher.fetch(&DocumentType::Rfc(9000)).await.unwrap();

    assert_eq!(content, "<html>RFC 9000 HTML</html>");
    assert_eq!(format, Format::Html);
}

#[tokio::test]
async fn fetch_rfc_both_404_returns_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rfc/rfc9000.txt"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rfc/rfc9000.html"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let fetcher = DocumentFetcher::with_client_and_urls(
        test_client(),
        &server.uri(),
        &server.uri(),
        &server.uri(),
    );
    let result = fetcher.fetch(&DocumentType::Rfc(9000)).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn fetch_draft_resolves_version_and_fetches_text() {
    let server = MockServer::start().await;
    // Draft version resolution: unversioned draft → returns rev "05"
    Mock::given(method("GET"))
        .and(path("/doc/draft-foo/doc.json"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"rev": "05"}"#,
            "application/json",
        ))
        .mount(&server)
        .await;
    // After resolution, fetch text for draft-foo-05.txt
    Mock::given(method("GET"))
        .and(path("/draft-foo-05.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_string("Draft content"))
        .mount(&server)
        .await;

    let fetcher = DocumentFetcher::with_client_and_urls(
        test_client(),
        &server.uri(),
        &server.uri(),
        &server.uri(),
    );
    let (content, format) = fetcher
        .fetch(&DocumentType::Draft("draft-foo".to_string()))
        .await
        .unwrap();

    assert_eq!(content, "Draft content");
    assert_eq!(format, Format::Text);
}

#[tokio::test]
async fn fetch_draft_not_found_returns_error() {
    let server = MockServer::start().await;
    // Version resolution fails
    Mock::given(method("GET"))
        .and(path("/doc/draft-nonexistent/doc.json"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let fetcher = DocumentFetcher::with_client_and_urls(
        test_client(),
        &server.uri(),
        &server.uri(),
        &server.uri(),
    );
    let result = fetcher
        .fetch(&DocumentType::Draft("draft-nonexistent".to_string()))
        .await;

    assert!(result.is_err());
}
