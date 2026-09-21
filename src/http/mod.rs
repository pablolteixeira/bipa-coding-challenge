//! HTTP API: `GET /nodes` and `GET /health`.

mod error;
mod response;

pub use error::ApiError;
pub use response::{ErrorBody, NodeResponse};
