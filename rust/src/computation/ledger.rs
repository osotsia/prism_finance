//! ledger.rs
//! Supports Value Enum for hybrid Scalar/Vector storage.

use crate::graph::NodeId;
use std::sync::Arc;

pub use self::error::ComputationError;
mod error {
    use super::*;
    use thiserror::Error;
    
    #[derive(Error, Debug, Clone, PartialEq)]
    pub enum ComputationError {
        #[error("Division by zero at node '{node_name}'")]
        DivisionByZero { node_id: NodeId, node_name: String },
        #[error("Upstream dependency '{parent_name}' of node '{node_name}' failed")]
        UpstreamError { node_id: NodeId, node_name: String, parent_id: NodeId, parent_name: String, source_error: Box<ComputationError> },
        #[error("Solver failed: {0}")]
        SolverDidNotConverge(String),
        #[error("Solver config error: {0}")]
        SolverConfiguration(String),
        #[error("Cycle detected")]
        CycleDetected,
        #[error("Parent count mismatch at '{node_name}'")]
        ParentCountMismatch { node_id: NodeId, node_name: String, expected: usize, actual: usize },
    }
}

/// The atomic unit of data in the engine.
/// This enum allows scalar math to occur without heap allocations.
#[derive(Debug, Clone)]
pub enum Value {
    Scalar(f64),
    /// Shared reference to a vector (TimeSeries).
    Series(Arc<Vec<f64>>),
}

impl Value {
    /// Helper to convert any Value to a vector (for Python/Solver consumption).
    /// This clones data if it is a scalar, which is expected at the boundary.
    pub fn to_vec(&self) -> Vec<f64> {
        match self {
            Value::Scalar(s) => vec![*s],
            Value::Series(s) => s.to_vec(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SolverIteration {
    pub iter_count: i32,
    pub obj_value: f64,
    pub inf_pr: f64,
    pub inf_du: f64,
}

#[derive(Debug, Clone, Default)]
pub struct Ledger {
    // Dense storage of Results of Values
    values: Vec<Option<Result<Value, ComputationError>>>,
    pub solver_trace: Option<Vec<SolverIteration>>,
}

impl Ledger {
    pub fn new() -> Self { Self::default() }

    pub fn ensure_capacity(&mut self, size: usize) {
        if self.values.len() < size {
            self.values.resize(size, None);
        }
    }

    #[inline(always)]
    pub fn get(&self, node_id: NodeId) -> Option<&Result<Value, ComputationError>> {
        self.values.get(node_id.index())?.as_ref()
    }

    #[inline(always)]
    pub fn insert(&mut self, node_id: NodeId, value: Result<Value, ComputationError>) {
        let idx = node_id.index();
        if idx >= self.values.len() {
            self.values.resize(idx + 1, None);
        }
        self.values[idx] = Some(value);
    }

    pub fn invalidate(&mut self, node_ids: impl IntoIterator<Item = NodeId>) {
        for id in node_ids {
            if let Some(slot) = self.values.get_mut(id.index()) {
                *slot = None;
            }
        }
    }
    
    pub fn is_timeseries(&self, node_ids: &[NodeId]) -> bool {
        node_ids.iter().any(|id| {
             match self.get(*id) {
                 Some(Ok(Value::Series(v))) => v.len() > 1,
                 _ => false,
             }
        })
    }
}