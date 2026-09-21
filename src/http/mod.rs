//! HTTP API: `GET /nodes` and `GET /health`.

mod error;
mod handlers;
mod response;

use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::get;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::storage::NodeRepository;

pub use error::ApiError;
pub use response::{ErrorBody, NodeResponse};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Shared state handed to handlers.
pub struct AppState<R> {
    repo: Arc<R>,
}

// Manual impl: cloning only bumps the Arc, so R needn't be Clone.
impl<R> Clone for AppState<R> {
    fn clone(&self) -> Self {
        Self {
            repo: Arc::clone(&self.repo),
        }
    }
}

/// Builds the application router on top of `repo`.
pub fn router<R>(repo: Arc<R>) -> Router
where
    R: NodeRepository + Send + Sync + 'static,
{
    Router::new()
        .route("/nodes", get(handlers::list_nodes::<R>))
        .route("/health", get(handlers::health))
        .fallback(handlers::not_found)
        .with_state(AppState { repo })
        // Layers wrap everything added before them, so the last one is the
        // outermost: tracing sees the final status, including panics and timeouts.
        .layer(CatchPanicLayer::custom(panic_response))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT,
        ))
        .layer(TraceLayer::new_for_http())
}

/// Turns a panic inside a handler into a JSON 500 instead of dropping the
/// connection. The server keeps serving other requests either way.
fn panic_response(panic: Box<dyn Any + Send + 'static>) -> Response {
    let message = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .unwrap_or("unknown panic");
    tracing::error!(panic = message, "handler panicked");
    error::internal_error_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{FakeRepository, node};
    use axum::body::Body;
    use axum::http::{Method, Request, header};
    use http_body_util::BodyExt;
    use serde_json::Value;
    use tower::ServiceExt;

    /// Sends one request through the router and returns status, content type
    /// and body.
    async fn send(app: Router, method: Method, uri: &str) -> (StatusCode, Option<String>, String) {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .map(|v| v.to_str().unwrap().to_owned());
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            content_type,
            String::from_utf8(body.to_vec()).unwrap(),
        )
    }

    fn challenge_nodes() -> Vec<crate::domain::Node> {
        vec![
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
        ]
    }

    const CHALLENGE_RESPONSE: &str = concat!(
        r#"[{"public_key":"03864ef025fde8fb587d989186ce6a4a186895ee44a926bfc370e2c366597a3f8f","#,
        r#""alias":"ACINQ","capacity":"360.10516297","first_seen":"2018-04-05T15:13:42Z"},"#,
        r#"{"public_key":"035e4ff418fc8b5554c5d9eea66396c227bd429a3251c8cbc711002ba215bfc226","#,
        r#""alias":"WalletOfSatoshi.com","capacity":"154.64503162","first_seen":"2020-09-30T01:39:00Z"}]"#
    );

    #[tokio::test]
    async fn nodes_returns_challenge_example() {
        let app = router(Arc::new(FakeRepository::with_nodes(challenge_nodes())));

        let (status, content_type, body) = send(app, Method::GET, "/nodes").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/json"));
        assert_eq!(body, CHALLENGE_RESPONSE);
    }

    #[tokio::test]
    async fn nodes_is_empty_array_before_first_import() {
        let app = router(Arc::new(FakeRepository::default()));

        let (status, _, body) = send(app, Method::GET, "/nodes").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "[]");
    }

    #[tokio::test]
    async fn nodes_objects_have_exactly_the_public_fields() {
        let app = router(Arc::new(FakeRepository::with_nodes(challenge_nodes())));

        let (_, _, body) = send(app, Method::GET, "/nodes").await;

        let json: Vec<serde_json::Map<String, Value>> = serde_json::from_str(&body).unwrap();
        for object in json {
            let mut keys: Vec<_> = object.keys().map(String::as_str).collect();
            keys.sort_unstable();
            assert_eq!(keys, ["alias", "capacity", "first_seen", "public_key"]);
        }
    }

    #[tokio::test]
    async fn nodes_preserves_repository_order() {
        let nodes = vec![
            node("z", "Z", 1, 0),
            node("a", "A", 2, 0),
            node("m", "M", 3, 0),
        ];
        let app = router(Arc::new(FakeRepository::with_nodes(nodes)));

        let (_, _, body) = send(app, Method::GET, "/nodes").await;

        let json: Vec<Value> = serde_json::from_str(&body).unwrap();
        let keys: Vec<_> = json
            .iter()
            .map(|n| n["public_key"].as_str().unwrap())
            .collect();
        assert_eq!(keys, ["z", "a", "m"]);
    }

    #[tokio::test]
    async fn nodes_escapes_but_keeps_alias_unchanged() {
        let alias = "DiamondHands💎🙌 \"quoted\" \\ | torq.co";
        let app = router(Arc::new(FakeRepository::with_nodes(vec![node(
            "k", alias, 1, 0,
        )])));

        let (_, _, body) = send(app, Method::GET, "/nodes").await;

        let json: Vec<Value> = serde_json::from_str(&body).unwrap();
        assert_eq!(json[0]["alias"], alias);
    }

    #[tokio::test]
    async fn nodes_database_failure_is_generic_500() {
        let repo = FakeRepository::default();
        repo.set_failing(true);
        let app = router(Arc::new(repo));

        let (status, content_type, body) = send(app, Method::GET, "/nodes").await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(content_type.as_deref(), Some("application/json"));
        assert_eq!(body, r#"{"error":"internal server error"}"#);
    }

    #[tokio::test]
    async fn panic_in_handler_is_500_and_server_keeps_serving() {
        let repo = Arc::new(FakeRepository::with_nodes(challenge_nodes()));
        let app = router(Arc::clone(&repo));
        repo.set_panic_on_list(true);

        let (status, _, body) = send(app.clone(), Method::GET, "/nodes").await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body, r#"{"error":"internal server error"}"#);

        repo.set_panic_on_list(false);
        let (status, _, body) = send(app, Method::GET, "/nodes").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, CHALLENGE_RESPONSE);
    }

    #[tokio::test]
    async fn health_is_ok_without_touching_the_database() {
        let repo = FakeRepository::default();
        repo.set_failing(true);
        let app = router(Arc::new(repo));

        let (status, _, body) = send(app, Method::GET, "/health").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, r#"{"status":"ok"}"#);
    }

    #[tokio::test]
    async fn unknown_route_is_json_404() {
        let app = router(Arc::new(FakeRepository::default()));

        let (status, content_type, body) = send(app, Method::GET, "/does-not-exist").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(content_type.as_deref(), Some("application/json"));
        assert_eq!(body, r#"{"error":"not found"}"#);
    }

    #[tokio::test]
    async fn wrong_method_is_405() {
        let app = router(Arc::new(FakeRepository::default()));

        let (status, _, _) = send(app, Method::POST, "/nodes").await;

        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    }
}
