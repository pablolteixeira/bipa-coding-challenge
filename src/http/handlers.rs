//! Request handlers.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde_json::{Value, json};

use super::AppState;
use super::error::ApiError;
use super::response::{ErrorBody, NodeResponse};
use crate::storage::NodeRepository;

/// `GET /nodes`: the stored ranking, formatted for clients.
///
/// Reads only from the database; the external API is never called here.
pub async fn list_nodes<R>(
    State(state): State<AppState<R>>,
) -> Result<Json<Vec<NodeResponse>>, ApiError>
where
    R: NodeRepository + Send + Sync + 'static,
{
    let nodes = state.repo.list_nodes().await?;
    Ok(Json(nodes.iter().map(NodeResponse::from).collect()))
}

/// `GET /health`: liveness only. It deliberately doesn't query the database,
/// so a database outage doesn't make orchestrators restart a healthy process.
pub async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

pub async fn not_found() -> (StatusCode, Json<ErrorBody>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorBody { error: "not found" }),
    )
}
