//! Background import of the node ranking from the source into storage.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;

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

    /// Imports immediately, then once per `interval`, until `token` is
    /// cancelled.
    ///
    /// No failure stops the loop: errors are logged and the next tick tries
    /// again. Each iteration runs in its own task, so even a panic inside a
    /// dependency surfaces as a logged [`ImportError::Panicked`] instead of
    /// killing the importer.
    pub async fn run(self, interval: Duration, token: CancellationToken) {
        let mut ticker = tokio::time::interval(interval);
        // If an import overruns the interval, wait a full interval afterwards
        // instead of firing a burst of catch-up imports.
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        tracing::info!(?interval, "importer started");
        loop {
            tokio::select! {
                // Check cancellation first so shutdown wins over a ready tick.
                biased;
                () = token.cancelled() => break,
                _ = ticker.tick() => {}
            }

            let started = Instant::now();
            let importer = self.clone();
            let result = match tokio::spawn(async move { importer.run_once().await }).await {
                Ok(result) => result,
                Err(join_error) => Err(ImportError::Panicked(join_error.to_string())),
            };
            log_result(&result, started.elapsed());
        }
        tracing::info!("importer stopped");
    }
}

fn log_result(result: &Result<ImportOutcome, ImportError>, elapsed: Duration) {
    match result {
        Ok(outcome) => tracing::info!(imported = outcome.imported, ?elapsed, "import succeeded"),
        // Upstream problems are expected from time to time: warn and retry.
        Err(err @ (ImportError::Source(_) | ImportError::EmptySource)) => {
            tracing::warn!(error = %err, ?elapsed, "import failed, keeping previous snapshot");
        }
        Err(err @ (ImportError::Storage(_) | ImportError::Panicked(_))) => {
            tracing::error!(error = %err, ?elapsed, "import failed, keeping previous snapshot");
        }
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

    const INTERVAL: Duration = Duration::from_secs(60);

    /// Waits (in virtual time) until `source` has been called `calls` times.
    async fn wait_for_calls(source: &FakeSource, calls: usize) {
        while source.calls() < calls {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    fn spawn_run(
        importer: Importer<FakeSource, FakeRepository>,
    ) -> (tokio::task::JoinHandle<()>, CancellationToken) {
        let token = CancellationToken::new();
        let handle = tokio::spawn(importer.run(INTERVAL, token.clone()));
        (handle, token)
    }

    // With `start_paused`, tokio's clock only moves when every task is idle and
    // jumps straight to the next timer, so these tests are exact and instant.

    #[tokio::test(start_paused = true)]
    async fn run_imports_immediately_on_start() {
        let (importer, source, repo) = importer(
            FakeSource::new([Step::Nodes(vec![node("a", "A", 1, 0)])]),
            FakeRepository::default(),
        );
        let (handle, token) = spawn_run(importer);

        tokio::time::timeout(INTERVAL / 2, wait_for_calls(&source, 1))
            .await
            .expect("first import should not wait for the interval");
        assert_eq!(repo.stored().len(), 1);

        token.cancel();
        handle.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn run_imports_on_every_tick() {
        let (importer, source, _) = importer(FakeSource::default(), FakeRepository::default());
        let (handle, token) = spawn_run(importer);

        tokio::time::sleep(INTERVAL * 3 + Duration::from_secs(1)).await;
        assert_eq!(source.calls(), 4, "initial import + 3 ticks");

        token.cancel();
        handle.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn run_recovers_after_failures() {
        let nodes = vec![node("a", "A", 1, 0)];
        let (importer, source, repo) = importer(
            FakeSource::new([Step::Fail(500), Step::Fail(503), Step::Nodes(nodes.clone())]),
            FakeRepository::default(),
        );
        let (handle, token) = spawn_run(importer);

        wait_for_calls(&source, 3).await;
        assert_eq!(repo.stored(), nodes);

        token.cancel();
        handle.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn run_survives_a_panicking_iteration() {
        let nodes = vec![node("a", "A", 1, 0)];
        let (importer, source, repo) = importer(
            FakeSource::new([Step::Panic, Step::Nodes(nodes.clone())]),
            FakeRepository::default(),
        );
        let (handle, token) = spawn_run(importer);

        wait_for_calls(&source, 2).await;
        assert_eq!(repo.stored(), nodes);
        assert!(
            !handle.is_finished(),
            "loop must keep running after a panic"
        );

        token.cancel();
        handle.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn run_stops_promptly_when_cancelled_while_waiting() {
        let (importer, source, _) = importer(FakeSource::default(), FakeRepository::default());
        let (handle, token) = spawn_run(importer);
        wait_for_calls(&source, 1).await;

        token.cancel();

        tokio::time::timeout(Duration::from_secs(1), handle)
            .await
            .expect("run should return well before the next tick")
            .unwrap();
        assert_eq!(source.calls(), 1);
    }

    #[tokio::test]
    async fn run_does_not_start_when_already_cancelled() {
        let (importer, source, _) = importer(FakeSource::default(), FakeRepository::default());
        let token = CancellationToken::new();
        token.cancel();

        importer.run(INTERVAL, token).await;

        assert_eq!(source.calls(), 0);
    }
}
