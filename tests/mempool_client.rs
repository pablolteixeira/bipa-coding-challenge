//! Integration tests for `MempoolClient` against a local wiremock server.

// Test helpers are plain functions, so clippy's test exemption doesn't cover them.
#![allow(clippy::unwrap_used)]

use std::time::{Duration, Instant};

use bipa_nodes::source::{MempoolClient, NodeSource, SourceError};
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const RANKINGS_PATH: &str = "/api/v1/lightning/nodes/rankings/connectivity";
const FIXTURE: &str = include_str!("fixtures/mempool_rankings.json");

fn client_for(base: &str, timeout: Duration) -> MempoolClient {
    let url = Url::parse(&format!("{base}{RANKINGS_PATH}")).unwrap();
    MempoolClient::new(url, timeout).unwrap()
}

async fn server_responding(response: ResponseTemplate) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(RANKINGS_PATH))
        .respond_with(response)
        .mount(&server)
        .await;
    server
}

fn json_body(body: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(body.to_owned(), "application/json")
}

#[tokio::test]
async fn fetches_and_validates_fixture_in_order() {
    let server = server_responding(json_body(FIXTURE)).await;
    let client = client_for(&server.uri(), Duration::from_secs(5));

    let nodes = client.fetch_nodes().await.unwrap();

    let aliases: Vec<_> = nodes.iter().map(|n| n.alias.as_str()).collect();
    assert_eq!(
        aliases,
        [
            "ACINQ",
            "1ML.com node ALPHA",
            "CoinGate",
            "DiamondHands💎🙌",
            "VIVA"
        ]
    );

    let acinq = &nodes[0];
    assert_eq!(
        acinq.public_key,
        "03864ef025fde8fb587d989186ce6a4a186895ee44a926bfc370e2c366597a3f8f"
    );
    assert_eq!(acinq.capacity_sats, 36_101_511_053);
    assert_eq!(acinq.first_seen.timestamp(), 1_522_941_222);
}

#[tokio::test]
async fn empty_array_is_ok_and_empty() {
    let server = server_responding(json_body("[]")).await;
    let client = client_for(&server.uri(), Duration::from_secs(5));

    assert!(client.fetch_nodes().await.unwrap().is_empty());
}

#[tokio::test]
async fn invalid_records_are_skipped() {
    let body = r#"[
        {"publicKey": "a", "alias": "ok", "capacity": 1, "firstSeen": 0},
        {"publicKey": "b", "alias": "negative", "capacity": -1, "firstSeen": 0},
        {"publicKey": "c", "alias": "missing"}
    ]"#;
    let server = server_responding(json_body(body)).await;
    let client = client_for(&server.uri(), Duration::from_secs(5));

    let nodes = client.fetch_nodes().await.unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].alias, "ok");
}

#[tokio::test]
async fn server_error_is_reported_as_status() {
    let server =
        server_responding(ResponseTemplate::new(500).set_body_string("<html>oops</html>")).await;
    let client = client_for(&server.uri(), Duration::from_secs(5));

    let err = client.fetch_nodes().await.unwrap_err();
    assert!(matches!(err, SourceError::Status(500)), "{err:?}");
}

#[tokio::test]
async fn rate_limit_is_reported_as_status() {
    let server = server_responding(ResponseTemplate::new(429)).await;
    let client = client_for(&server.uri(), Duration::from_secs(5));

    let err = client.fetch_nodes().await.unwrap_err();
    assert!(matches!(err, SourceError::Status(429)), "{err:?}");
}

#[tokio::test]
async fn slow_response_times_out() {
    let server = server_responding(json_body(FIXTURE).set_delay(Duration::from_secs(10))).await;
    let client = client_for(&server.uri(), Duration::from_secs(1));

    let started = Instant::now();
    let err = client.fetch_nodes().await.unwrap_err();

    assert!(
        matches!(err, SourceError::Request(ref e) if e.is_timeout()),
        "{err:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "took {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn non_array_json_is_a_decode_error() {
    let server = server_responding(json_body(r#"{"not": "an array"}"#)).await;
    let client = client_for(&server.uri(), Duration::from_secs(5));

    let err = client.fetch_nodes().await.unwrap_err();
    assert!(matches!(err, SourceError::Decode(_)), "{err:?}");
}

#[tokio::test]
async fn garbage_body_is_a_decode_error() {
    let server = server_responding(json_body("garbage")).await;
    let client = client_for(&server.uri(), Duration::from_secs(5));

    let err = client.fetch_nodes().await.unwrap_err();
    assert!(matches!(err, SourceError::Decode(_)), "{err:?}");
}

#[tokio::test]
async fn unreachable_server_is_a_request_error() {
    // Bind to get a free port, then release it so nothing is listening there.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let client = client_for(&format!("http://{addr}"), Duration::from_secs(5));

    let err = client.fetch_nodes().await.unwrap_err();
    assert!(matches!(err, SourceError::Request(_)), "{err:?}");
}

/// Hits the real mempool.space. Run with `cargo test -- --ignored`.
#[tokio::test]
#[ignore = "requires internet access"]
async fn live_mempool_space() {
    let url = Url::parse(&format!("https://mempool.space{RANKINGS_PATH}")).unwrap();
    let client = MempoolClient::new(url, Duration::from_secs(20)).unwrap();

    let nodes = client.fetch_nodes().await.unwrap();

    assert!(!nodes.is_empty());
    assert!(nodes.iter().all(|n| !n.public_key.is_empty()));
}
