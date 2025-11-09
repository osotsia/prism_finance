//! The Ledger stores computed f64 values, acting as a memoization cache.
use crate::graph::NodeId;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// A structured error report from the computation engine.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum ComputationError {
    #[error("Division by zero at node '{node_name}' ({node_id:?})")]
    DivisionByZero { node_id: NodeId, node_name: String },

    #[error("Upstream dependency '{parent_name}' ({parent_id:?}) of node '{node_name}' failed")]
    UpstreamError {
        node_id: NodeId,
        node_name: String,
        parent_id: NodeId,
        parent_name: String,
        source_error: Box<ComputationError>,
    },

    #[error("Solver failed to converge: {0}")]
    SolverDidNotConverge(String),

    #[error("Solver configuration error: {0}")]
    SolverConfiguration(String),

    #[error("Graph contains a cycle")]
    CycleDetected,

    #[error("Node '{node_name}' ({node_id:?}) expected {expected} parents for operation {op:?}, but got {actual}")]
    ParentCountMismatch { node_id: NodeId, node_name: String, op: String, expected: usize, actual: usize },
}

/// The "bookkeeper" of the engine. It records the final value for each
/// node once calculated.
#[derive(Debug, Clone, Default)]
pub struct Ledger {
    values: HashMap<NodeId, Result<Arc<Vec<f64>>, ComputationError>>,
}

impl Ledger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, node_id: NodeId) -> Option<&Result<Arc<Vec<f64>>, ComputationError>> {
        self.values.get(&node_id)
    }

    pub fn insert(&mut self, node_id: NodeId, value: Result<Arc<Vec<f64>>, ComputationError>) {
        self.values.insert(node_id, value);
    }

    pub fn invalidate(&mut self, node_ids: impl IntoIterator<Item = NodeId>) {
        for id in node_ids {
            self.values.remove(&id);
        }
    }

    /// Checks if any node in a given list corresponds to a time-series vector
    /// in the ledger.
    pub fn is_timeseries(&self, node_ids: &[NodeId]) -> bool {
        node_ids.iter().any(|id| {
            if let Some(Ok(val)) = self.values.get(id) {
                val.len() > 1
            } else {
                // If not in ledger, assume scalar. This path shouldn't be hit
                // for solver vars, as they get default values.
                false
            }
        })
    }
}