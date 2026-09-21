//! Integration tests for the importer with real components: a wiremock
//! server standing in for mempool.space and a real PostgreSQL database.

// Test helpers are plain functions, so clippy's test exemption doesn't cover them.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

use bipa_nodes::importer::{ImportError, Importer};
use bipa_nodes::source::{MempoolClient, SourceError};
use bipa_nodes::storage::{NodeRepository, PgNodeRepository};
use sqlx::PgPool;
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const RANKINGS_PATH: &str = "/api/v1/lightning/nodes/rankings/connectivity";
const FIXTURE: &str = include_str!("fixtures/mempool_rankings.json");

struct Harness {
    server: MockServer,
    importer: Importer<MempoolClient, PgNodeRepository>,
    repo: Arc<PgNodeRepository>,
}

impl Harness {
    async fn new(pool: PgPool) -> Self {
        let server = MockServer::start().await;
        let url = Url::parse(&format!("{}{RANKINGS_PATH}", server.uri())).unwrap();
        let source = Arc::new(MempoolClient::new(url, Duration::from_secs(5)).unwrap());
        let repo = Arc::new(PgNodeRepository::new(pool));
        let importer = Importer::new(source, Arc::clone(&repo));
        Self {
            server,
            importer,
            repo,
        }
    }

    /// Replaces whatever the fake mempool.space currently responds with.
    async fn respond_with(&self, response: ResponseTemplate) {
        self.server.reset().await;
        Mock::given(method("GET"))
            .and(path(RANKINGS_PATH))
            .respond_with(response)
            .mount(&self.server)
            .await;
    }

    async fn stored_aliases(&self) -> Vec<String> {
        self.repo
            .list_nodes()
            .await
            .unwrap()
            .into_iter()
            .map(|n| n.alias)
            .collect()
    }
}

fn json_body(body: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(body.to_owned(), "application/json")
}

#[sqlx::test]
async fn imports_fixture_into_database(pool: PgPool) {
    let harness = Harness::new(pool).await;
    harness.respond_with(json_body(FIXTURE)).await;

    let outcome = harness.importer.run_once().await.unwrap();

    assert_eq!(outcome.imported, 5);
    assert_eq!(
        harness.stored_aliases().await,
        [
            "ACINQ",
            "1ML.com node ALPHA",
            "CoinGate",
            "DiamondHands💎🙌",
            "VIVA"
        ]
    );
}

#[sqlx::test]
async fn upstream_failure_keeps_previous_snapshot(pool: PgPool) {
    let harness = Harness::new(pool).await;
    harness.respond_with(json_body(FIXTURE)).await;
    harness.importer.run_once().await.unwrap();
    let before = harness.stored_aliases().await;

    harness.respond_with(ResponseTemplate::new(500)).await;
    let err = harness.importer.run_once().await.unwrap_err();

    assert!(
        matches!(err, ImportError::Source(SourceError::Status(500))),
        "{err:?}"
    );
    assert_eq!(harness.stored_aliases().await, before);
}

#[sqlx::test]
async fn empty_upstream_response_keeps_previous_snapshot(pool: PgPool) {
    let harness = Harness::new(pool).await;
    harness.respond_with(json_body(FIXTURE)).await;
    harness.importer.run_once().await.unwrap();
    let before = harness.stored_aliases().await;

    harness.respond_with(json_body("[]")).await;
    let err = harness.importer.run_once().await.unwrap_err();

    assert!(matches!(err, ImportError::EmptySource), "{err:?}");
    assert_eq!(harness.stored_aliases().await, before);
}

#[sqlx::test]
async fn ranking_change_replaces_stored_ranking(pool: PgPool) {
    let harness = Harness::new(pool).await;
    harness.respond_with(json_body(FIXTURE)).await;
    harness.importer.run_once().await.unwrap();

    let new_ranking = r#"[
        {"publicKey": "02new", "alias": "Newcomer", "capacity": 10, "firstSeen": 1700000000},
        {"publicKey": "03864ef025fde8fb587d989186ce6a4a186895ee44a926bfc370e2c366597a3f8f",
         "alias": "ACINQ", "capacity": 36101511053, "firstSeen": 1522941222}
    ]"#;
    harness.respond_with(json_body(new_ranking)).await;
    harness.importer.run_once().await.unwrap();

    assert_eq!(harness.stored_aliases().await, ["Newcomer", "ACINQ"]);
}
