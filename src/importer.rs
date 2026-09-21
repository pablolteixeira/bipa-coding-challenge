//! Background import of the node ranking from the source into storage.

use std::sync::Arc;

use crate::source::{NodeSource, SourceError};
use crate::storage::{NodeRepository, StorageError};

/// Copies the current ranking from a [`NodeSource`] into a [`NodeRepository`].
pub struct Importer<S, R> {
    source: Arc<S>,
    repo: Arc<R>,
}

// Manual impl: cloning only bumps the Arcs, so S and R needn't be Clone.
impl<S, R> Clone for Importer<S, R> {
    fn clone(&self) -> Self {
        Self {
            source: Arc::clone(&self.source),
            repo: Arc::clone(&self.repo),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportOutcome {
    pub imported: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error(transparent)]
    Source(#[from] SourceError),
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// The source returned no valid nodes. The ranking is never legitimately
    /// empty, so this is treated as an upstream problem.
    #[error("node source returned no valid nodes; keeping the previous snapshot")]
    EmptySource,
    #[error("import task panicked: {0}")]
    Panicked(String),
}

impl<S, R> Importer<S, R>
where
    S: NodeSource + Send + Sync + 'static,
    R: NodeRepository + Send + Sync + 'static,
{
    pub fn new(source: Arc<S>, repo: Arc<R>) -> Self {
        Self { source, repo }
    }

    /// Fetches the ranking once and replaces the stored snapshot with it.
    ///
    /// Storage is only touched after a successful, non-empty fetch, so any
    /// failure leaves the last good snapshot in place.
    pub async fn run_once(&self) -> Result<ImportOutcome, ImportError> {
        let nodes = self.source.fetch_nodes().await?;
        if nodes.is_empty() {
            return Err(ImportError::EmptySource);
        }
        let imported = self.repo.replace_all(&nodes).await?;
        Ok(ImportOutcome { imported })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{FakeRepository, FakeSource, Step, node};

    fn importer(
        source: FakeSource,
        repo: FakeRepository,
    ) -> (
        Importer<FakeSource, FakeRepository>,
        Arc<FakeSource>,
        Arc<FakeRepository>,
    ) {
        let source = Arc::new(source);
        let repo = Arc::new(repo);
        (
            Importer::new(Arc::clone(&source), Arc::clone(&repo)),
            source,
            repo,
        )
    }

    #[tokio::test]
    async fn run_once_stores_fetched_nodes_in_order() {
        let nodes = vec![
            node("b", "B", 2, 0),
            node("a", "A", 1, 0),
            node("c", "C", 3, 0),
        ];
        let (importer, _, repo) = importer(
            FakeSource::new([Step::Nodes(nodes.clone())]),
            FakeRepository::default(),
        );

        let outcome = importer.run_once().await.unwrap();

        assert_eq!(outcome, ImportOutcome { imported: 3 });
        assert_eq!(repo.replace_calls(), vec![nodes.clone()]);
        assert_eq!(repo.stored(), nodes);
    }

    #[tokio::test]
    async fn run_once_source_error_does_not_touch_storage() {
        let previous = vec![node("old", "Old", 1, 0)];
        let (importer, _, repo) = importer(
            FakeSource::new([Step::Fail(500)]),
            FakeRepository::with_nodes(previous.clone()),
        );

        let err = importer.run_once().await.unwrap_err();

        assert!(
            matches!(err, ImportError::Source(SourceError::Status(500))),
            "{err:?}"
        );
        assert!(repo.replace_calls().is_empty());
        assert_eq!(repo.stored(), previous);
    }

    #[tokio::test]
    async fn run_once_empty_source_keeps_previous_snapshot() {
        let previous = vec![node("old", "Old", 1, 0)];
        let (importer, _, repo) = importer(
            FakeSource::new([Step::Nodes(Vec::new())]),
            FakeRepository::with_nodes(previous.clone()),
        );

        let err = importer.run_once().await.unwrap_err();

        assert!(matches!(err, ImportError::EmptySource), "{err:?}");
        assert!(repo.replace_calls().is_empty());
        assert_eq!(repo.stored(), previous);
    }

    #[tokio::test]
    async fn run_once_reports_storage_error() {
        let repo = FakeRepository::default();
        repo.set_failing(true);
        let (importer, _, _) = importer(
            FakeSource::new([Step::Nodes(vec![node("a", "A", 1, 0)])]),
            repo,
        );

        let err = importer.run_once().await.unwrap_err();

        assert!(matches!(err, ImportError::Storage(_)), "{err:?}");
    }
}
