//! Core domain types and pure conversions. Depends on nothing else in the crate.

mod node;

pub use node::Node;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("unix timestamp {0} is out of the representable range")]
    InvalidTimestamp(i64),
}
