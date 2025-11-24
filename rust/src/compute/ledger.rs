use crate::store::NodeId;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum ComputationError {
    #[error("Math error: {0}")]
    MathError(String),
    #[error("Upstream error: {0}")]
    Upstream(String),
    #[error("Structural mismatch: {msg}")]
    Mismatch { msg: String },
    #[error("Cycle detected")]
    CycleDetected,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SolverIteration {
    pub iter_count: i32,
    pub obj_value: f64,
    pub inf_pr: f64,
    pub inf_du: f64,
}

#[derive(Debug, Clone)]
pub enum Value {
    Scalar(f64),
    Series(Arc<Vec<f64>>),
}

impl Value {
    pub fn len(&self) -> usize {
        match self { Value::Scalar(_) => 1, Value::Series(v) => v.len() }
    }
    
    pub fn shape(&self) -> (usize, bool) {
        match self { Value::Scalar(_) => (1, true), Value::Series(v) => (v.len(), false) }
    }

    #[inline(always)]
    pub fn get_at(&self, i: usize) -> f64 {
        match self {
            Value::Scalar(s) => *s,
            Value::Series(vec) => *vec.get(i).unwrap_or_else(|| vec.last().unwrap_or(&0.0))
        }
    }

    #[inline(always)]
    pub fn as_scalar_unchecked(&self) -> f64 {
        match self { Value::Scalar(s) => *s, _ => 0.0 }
    }

    pub fn to_vec(&self) -> Vec<f64> {
        match self { Value::Scalar(s) => vec![*s], Value::Series(s) => s.to_vec() }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Ledger {
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

    pub fn get(&self, node_id: NodeId) -> Option<&Result<Value, ComputationError>> {
        self.values.get(node_id.index())?.as_ref()
    }

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
}