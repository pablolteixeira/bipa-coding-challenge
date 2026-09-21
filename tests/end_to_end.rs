//! End-to-end tests of the whole service.
//!
//! The first group runs the service in-process (`app::serve`) on a random port,
//! with wiremock standing in for mempool.space and an isolated PostgreSQL
//! database per test. The second group runs the compiled binary to check exit
//! codes and signal handling.

// Test helpers are plain functions, so clippy's test exemption doesn't cover them.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::future::Future;
use std::net::SocketAddr;
use std::process::{Output, Stdio};
use std::time::Duration;

use bipa_nodes::app::{self, AppError};
use bipa_nodes::config::Config;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpListener;
use tokio::process::{Child, Command};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const RANKINGS_PATH: &str = "/api/v1/lightning/nodes/rankings/connectivity";
const FIXTURE: &str = include_str!("fixtures/mempool_rankings.json");
const FIXTURE_ALIASES: [&str; 5] = [
    "ACINQ",
    "1ML.com node ALPHA",
    "CoinGate",
    "DiamondHands💎🙌",
    "VIVA",
];
const DEADLINE: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn mock_mempool(response: ResponseTemplate) -> MockServer {
    let server = MockServer::start().await;
    respond_with(&server, response).await;
    server
}

/// Replaces the fake mempool.space response (and clears recorded requests).
async fn respond_with(server: &MockServer, response: ResponseTemplate) {
    server.reset().await;
    Mock::given(method("GET"))
        .and(path(RANKINGS_PATH))
        .respond_with(response)
        .mount(server)
        .await;
}

fn json_body(body: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(body.to_owned(), "application/json")
}

/// Polls `check` until it returns `Some`, failing the test after `DEADLINE`.
async fn eventually<T, F, Fut>(what: &str, mut check: F) -> T
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Option<T>>,
{
    let poll = async {
        loop {
            if let Some(value) = check().await {
                return value;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };
    tokio::time::timeout(DEADLINE, poll)
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
}

async fn get_json(addr: SocketAddr, route: &str) -> Option<(u16, Value)> {
    let response = reqwest::get(format!("http://{addr}{route}")).await.ok()?;
    let status = response.status().as_u16();
    Some((status, response.json().await.ok()?))
}

async fn aliases(addr: SocketAddr) -> Option<Vec<String>> {
    let (_, body) = get_json(addr, "/nodes").await?;
    Some(
        body.as_array()?
            .iter()
            .map(|n| n["alias"].as_str().unwrap_or_default().to_owned())
            .collect(),
    )
}

struct RunningService {
    addr: SocketAddr,
    token: CancellationToken,
    handle: JoinHandle<Result<(), AppError>>,
}

/// Starts the full service in-process on a random port.
async fn start_service(pool: PgPool, mempool: &MockServer) -> RunningService {
    let config = Config {
        // Unused by `serve`, which receives the test pool directly.
        database_url: String::new(),
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        mempool_url: Url::parse(&format!("{}{RANKINGS_PATH}", mempool.uri())).unwrap(),
        import_interval: Duration::from_millis(200),
        http_client_timeout: Duration::from_secs(2),
    };
    let listener = TcpListener::bind(config.bind_addr).await.unwrap();
    let addr = listener.local_addr().unwrap();
    let token = CancellationToken::new();
    let handle = tokio::spawn({
        let token = token.clone();
        async move { app::serve(&config, pool, listener, token).await }
    });
    RunningService {
        addr,
        token,
        handle,
    }
}

// ---------------------------------------------------------------------------
// In-process service
// ---------------------------------------------------------------------------

#[sqlx::test]
async fn serves_imported_nodes(pool: PgPool) {
    let mempool = mock_mempool(json_body(FIXTURE)).await;
    let service = start_service(pool, &mempool).await;

    let (status, health) = eventually("health", || get_json(service.addr, "/health")).await;
    assert_eq!(status, 200);
    assert_eq!(health, json!({ "status": "ok" }));

    let nodes = eventually("imported nodes", || async {
        let (_, body) = get_json(service.addr, "/nodes").await?;
        (body.as_array()?.len() == FIXTURE_ALIASES.len()).then_some(body)
    })
    .await;
    assert_eq!(
        nodes[0],
        json!({
            "public_key": "03864ef025fde8fb587d989186ce6a4a186895ee44a926bfc370e2c366597a3f8f",
            "alias": "ACINQ",
            "capacity": "361.01511053",
            "first_seen": "2018-04-05T15:13:42Z"
        })
    );
    assert_eq!(aliases(service.addr).await.unwrap(), FIXTURE_ALIASES);

    service.token.cancel();
    service.handle.await.unwrap().unwrap();
}

#[sqlx::test]
async fn keeps_serving_last_snapshot_during_upstream_outage(pool: PgPool) {
    let mempool = mock_mempool(json_body(FIXTURE)).await;
    let service = start_service(pool, &mempool).await;
    eventually("first import", || async {
        (aliases(service.addr).await? == FIXTURE_ALIASES).then_some(())
    })
    .await;

    respond_with(&mempool, ResponseTemplate::new(500)).await;
    // Wait for several failed import attempts against the broken upstream.
    eventually("failed imports", || async {
        (mempool.received_requests().await?.len() >= 3).then_some(())
    })
    .await;

    let (status, _) = get_json(service.addr, "/nodes").await.unwrap();
    assert_eq!(status, 200);
    assert_eq!(aliases(service.addr).await.unwrap(), FIXTURE_ALIASES);

    service.token.cancel();
    service.handle.await.unwrap().unwrap();
}

#[sqlx::test]
async fn reflects_ranking_changes(pool: PgPool) {
    let mempool = mock_mempool(json_body(FIXTURE)).await;
    let service = start_service(pool, &mempool).await;
    eventually("first import", || async {
        (aliases(service.addr).await? == FIXTURE_ALIASES).then_some(())
    })
    .await;

    let new_ranking = r#"[
        {"publicKey": "02aa", "alias": "Newcomer", "capacity": 100000000, "firstSeen": 1700000000},
        {"publicKey": "02bb", "alias": "Runner-up", "capacity": 550000, "firstSeen": 1600000000}
    ]"#;
    respond_with(&mempool, json_body(new_ranking)).await;

    eventually("new ranking", || async {
        (aliases(service.addr).await? == ["Newcomer", "Runner-up"]).then_some(())
    })
    .await;
    let (_, body) = get_json(service.addr, "/nodes").await.unwrap();
    assert_eq!(body[0]["capacity"], "1.00000000");
    assert_eq!(body[1]["capacity"], "0.00550000");

    service.token.cancel();
    service.handle.await.unwrap().unwrap();
}

#[sqlx::test]
async fn shuts_down_gracefully(pool: PgPool) {
    let mempool = mock_mempool(json_body(FIXTURE)).await;
    let service = start_service(pool, &mempool).await;
    eventually("health", || get_json(service.addr, "/health")).await;

    service.token.cancel();

    let result = tokio::time::timeout(Duration::from_secs(5), service.handle)
        .await
        .expect("service should stop within 5s")
        .unwrap();
    assert!(result.is_ok(), "{result:?}");
    assert!(
        tokio::net::TcpStream::connect(service.addr).await.is_err(),
        "port should no longer accept connections"
    );
}

// ---------------------------------------------------------------------------
// Compiled binary
// ---------------------------------------------------------------------------

fn free_port_addr() -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap()
}

fn spawn_binary(envs: &[(&str, String)]) -> Child {
    let mut command = Command::new(env!("CARGO_BIN_EXE_bipa-nodes"));
    command
        .env_clear()
        .env("NO_COLOR", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for (key, value) in envs {
        command.env(key, value);
    }
    command.spawn().unwrap()
}

async fn wait_for_exit(child: Child) -> (Output, String) {
    let output = tokio::time::timeout(DEADLINE, child.wait_with_output())
        .await
        .expect("binary should exit")
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output, text)
}

#[cfg(unix)]
fn send_sigterm(child: &Child) {
    let pid = child.id().expect("child should still be running");
    let status = std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .unwrap();
    assert!(status.success());
}

#[tokio::test]
async fn binary_exits_with_error_on_invalid_config() {
    let child = spawn_binary(&[
        (
            "DATABASE_URL",
            "postgres://bipa:bipa@127.0.0.1:1/bipa".to_owned(),
        ),
        ("IMPORT_INTERVAL_SECS", "0".to_owned()),
    ]);

    let (output, text) = wait_for_exit(child).await;

    assert_eq!(output.status.code(), Some(1), "{text}");
    assert!(text.contains("IMPORT_INTERVAL_SECS"), "{text}");
    assert!(!text.contains("panicked"), "{text}");
}

#[tokio::test]
async fn binary_exits_with_error_when_port_is_taken() {
    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let child = spawn_binary(&[
        (
            "DATABASE_URL",
            "postgres://bipa:bipa@127.0.0.1:1/bipa".to_owned(),
        ),
        ("BIND_ADDR", taken.local_addr().unwrap().to_string()),
    ]);

    let (output, text) = wait_for_exit(child).await;

    assert_eq!(output.status.code(), Some(1), "{text}");
    assert!(text.contains("failed to bind"), "{text}");
    assert!(!text.contains("panicked"), "{text}");
}

#[cfg(unix)]
#[tokio::test]
async fn binary_retries_unreachable_database_and_exits_cleanly_on_sigterm() {
    let mut child = spawn_binary(&[
        (
            "DATABASE_URL",
            "postgres://bipa:bipa@127.0.0.1:1/bipa".to_owned(),
        ),
        ("BIND_ADDR", free_port_addr().to_string()),
    ]);

    // sqlx itself keeps trying until the pool's acquire timeout (5s) before
    // reporting the first failure, so wait for the retry log line rather
    // than sleeping a fixed amount.
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut log = String::new();
    tokio::time::timeout(DEADLINE, async {
        while let Some(line) = lines.next_line().await.unwrap() {
            log.push_str(&line);
            log.push('\n');
            if line.contains("database connection failed, retrying") {
                return;
            }
        }
        panic!("binary exited before retrying:\n{log}");
    })
    .await
    .expect("binary should log a retry");

    send_sigterm(&child);
    let status = tokio::time::timeout(DEADLINE, child.wait())
        .await
        .expect("binary should exit after SIGTERM")
        .unwrap();
    while let Some(line) = lines.next_line().await.unwrap() {
        log.push_str(&line);
        log.push('\n');
    }

    assert_eq!(status.code(), Some(0), "{log}");
    assert!(!log.contains("panicked"), "{log}");
}

/// Runs the real binary against an isolated database and a fake upstream,
/// then stops it with SIGTERM.
#[cfg(unix)]
#[sqlx::test]
async fn binary_serves_nodes_and_stops_on_sigterm(
    _pool_options: PgPoolOptions,
    connect_options: PgConnectOptions,
) {
    // Point the binary at the per-test database sqlx created for us.
    let mut database_url = Url::parse(&std::env::var("DATABASE_URL").unwrap()).unwrap();
    database_url.set_path(connect_options.get_database().unwrap());

    let mempool = mock_mempool(json_body(FIXTURE)).await;
    let addr = free_port_addr();
    let child = spawn_binary(&[
        ("DATABASE_URL", database_url.to_string()),
        ("BIND_ADDR", addr.to_string()),
        ("MEMPOOL_URL", format!("{}{RANKINGS_PATH}", mempool.uri())),
    ]);

    eventually("binary to serve imported nodes", || async {
        (aliases(addr).await? == FIXTURE_ALIASES).then_some(())
    })
    .await;

    send_sigterm(&child);
    let (output, text) = wait_for_exit(child).await;

    assert_eq!(output.status.code(), Some(0), "{text}");
    assert!(text.contains("shutdown complete"), "{text}");
    assert!(!text.contains("panicked"), "{text}");
}
