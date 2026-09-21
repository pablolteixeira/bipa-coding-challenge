//! Integration tests for the HTTP API on top of a real PostgreSQL repository.

// Test helpers are plain functions, so clippy's test exemption doesn't cover them.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use bipa_nodes::domain::{Node, timestamp_from_unix};
use bipa_nodes::http;
use bipa_nodes::storage::{NodeRepository, PgNodeRepository};
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

async fn get(app: Router, uri: &str) -> (StatusCode, String) {
    let request = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

fn node(public_key: &str, alias: &str, capacity_sats: u64, first_seen: i64) -> Node {
    Node {
        public_key: public_key.to_owned(),
        alias: alias.to_owned(),
        capacity_sats,
        first_seen: timestamp_from_unix(first_seen).unwrap(),
    }
}

#[sqlx::test]
async fn nodes_returns_challenge_example_from_database(pool: PgPool) {
    let repo = Arc::new(PgNodeRepository::new(pool));
    repo.replace_all(&[
        node(
            "03864ef025fde8fb587d989186ce6a4a186895ee44a926bfc370e2c366597a3f8f",
            "ACINQ",
            36_010_516_297,
            1_522_941_222,
        ),
        node(
            "035e4ff418fc8b5554c5d9eea66396c227bd429a3251c8cbc711002ba215bfc226",
            "WalletOfSatoshi.com",
            15_464_503_162,
            1_601_429_940,
        ),
    ])
    .await
    .unwrap();

    let (status, body) = get(http::router(repo), "/nodes").await;

    assert_eq!(status, StatusCode::OK);
    let expected = serde_json::json!([
        {
            "public_key": "03864ef025fde8fb587d989186ce6a4a186895ee44a926bfc370e2c366597a3f8f",
            "alias": "ACINQ",
            "capacity": "360.10516297",
            "first_seen": "2018-04-05T15:13:42Z"
        },
        {
            "public_key": "035e4ff418fc8b5554c5d9eea66396c227bd429a3251c8cbc711002ba215bfc226",
            "alias": "WalletOfSatoshi.com",
            "capacity": "154.64503162",
            "first_seen": "2020-09-30T01:39:00Z"
        }
    ]);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap(),
        expected
    );
}

#[sqlx::test]
async fn nodes_is_empty_on_fresh_database(pool: PgPool) {
    let app = http::router(Arc::new(PgNodeRepository::new(pool)));

    let (status, body) = get(app, "/nodes").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "[]");
}

#[sqlx::test]
async fn database_unavailable_returns_500_without_panicking(pool: PgPool) {
    let app = http::router(Arc::new(PgNodeRepository::new(pool.clone())));
    pool.close().await;

    let (status, body) = get(app.clone(), "/nodes").await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body, r#"{"error":"internal server error"}"#);

    // The process is still healthy even though the database is gone.
    let (status, _) = get(app, "/health").await;
    assert_eq!(status, StatusCode::OK);
}
