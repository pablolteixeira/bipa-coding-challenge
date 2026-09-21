//! In-memory test doubles for the [`NodeSource`] and [`NodeRepository`] traits.

// Shared by several test modules; not every helper is used by each of them.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::domain::{Node, timestamp_from_unix};
use crate::source::{NodeSource, SourceError};
use crate::storage::{NodeRepository, StorageError};

pub fn node(public_key: &str, alias: &str, capacity_sats: u64, first_seen: i64) -> Node {
    Node {
        public_key: public_key.to_owned(),
        alias: alias.to_owned(),
        capacity_sats,
        first_seen: timestamp_from_unix(first_seen).unwrap(),
    }
}

/// What a [`FakeSource`] does on one call.
pub enum Step {
    Nodes(Vec<Node>),
    Fail(u16),
    Panic,
}

/// Source that replays a script of steps, then fails with 503 once exhausted.
#[derive(Default)]
pub struct FakeSource {
    script: Mutex<VecDeque<Step>>,
    calls: AtomicUsize,
}

impl FakeSource {
    pub fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            script: Mutex::new(steps.into_iter().collect()),
            calls: AtomicUsize::new(0),
        }
    }

    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl NodeSource for FakeSource {
    async fn fetch_nodes(&self) -> Result<Vec<Node>, SourceError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let step = self.script.lock().unwrap().pop_front();
        match step {
            Some(Step::Nodes(nodes)) => Ok(nodes),
            Some(Step::Fail(status)) => Err(SourceError::Status(status)),
            Some(Step::Panic) => panic!("scripted panic in FakeSource"),
            None => Err(SourceError::Status(503)),
        }
    }
}

/// Repository backed by a `Vec`, with switches to simulate failures.
#[derive(Default)]
pub struct FakeRepository {
    nodes: Mutex<Vec<Node>>,
    replace_calls: Mutex<Vec<Vec<Node>>>,
    fail: AtomicBool,
    panic_on_list: AtomicBool,
}

impl FakeRepository {
    pub fn with_nodes(nodes: Vec<Node>) -> Self {
        let repo = Self::default();
        *repo.nodes.lock().unwrap() = nodes;
        repo
    }

    /// Makes every subsequent call return a database error.
    pub fn set_failing(&self, fail: bool) {
        self.fail.store(fail, Ordering::SeqCst);
    }

    /// Makes `list_nodes` panic, to exercise panic isolation.
    pub fn set_panic_on_list(&self, panic: bool) {
        self.panic_on_list.store(panic, Ordering::SeqCst);
    }

    pub fn stored(&self) -> Vec<Node> {
        self.nodes.lock().unwrap().clone()
    }

    pub fn replace_calls(&self) -> Vec<Vec<Node>> {
        self.replace_calls.lock().unwrap().clone()
    }
}

impl NodeRepository for FakeRepository {
    async fn replace_all(&self, nodes: &[Node]) -> Result<usize, StorageError> {
        self.replace_calls.lock().unwrap().push(nodes.to_vec());
        if self.fail.load(Ordering::SeqCst) {
            return Err(StorageError::Database(sqlx::Error::PoolTimedOut));
        }
        *self.nodes.lock().unwrap() = nodes.to_vec();
        Ok(nodes.len())
    }

    async fn list_nodes(&self) -> Result<Vec<Node>, StorageError> {
        if self.panic_on_list.load(Ordering::SeqCst) {
            panic!("scripted panic in FakeRepository");
        }
        if self.fail.load(Ordering::SeqCst) {
            return Err(StorageError::Database(sqlx::Error::PoolTimedOut));
        }
        Ok(self.stored())
    }
}
