//! Mapping of internal failures to HTTP responses.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use super::response::ErrorBody;
use crate::storage::StorageError;

pub const INTERNAL_ERROR: &str = "internal server error";

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error(transparent)]
    Storage(#[from] StorageError),
}

impl IntoResponse for ApiError {
    /// Logs the details and returns a generic 500, so internal information
    /// (SQL errors, hostnames) never reaches clients.
    fn into_response(self) -> Response {
        tracing::error!(error = %self, "request failed");
        internal_error_response()
    }
}

pub fn internal_error_response() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorBody {
            error: INTERNAL_ERROR,
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    #[tokio::test]
    async fn storage_error_becomes_generic_500() {
        let error = ApiError::from(StorageError::Database(sqlx::Error::Protocol(
            "secret internal detail".to_owned(),
        )));

        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], br#"{"error":"internal server error"}"#);
    }
}
