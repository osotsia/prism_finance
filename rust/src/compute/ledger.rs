use crate::store::NodeId;
use std::sync::Arc;
use std::collections::HashMap;
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

/// A unifying wrapper for the two types of data Prism handles.
/// Used primarily for the public API and slow-path operations.
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

/// Efficient status tracking for the SoA layout.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum NodeStatus {
    Uncomputed = 0,
    ComputedScalar = 1,
    ComputedSeries = 2,
    Error = 3,
}

/// The DenseLedger organizes data in a Structure-of-Arrays (SoA) layout.
#[derive(Debug, Clone, Default)]
pub struct Ledger {
    // Primary storage (Fast Path): Contiguous f64 array.
    pub scalars: Vec<f64>,
    
    // Secondary storage (Slow Path): For time-series data.
    pub series: Vec<Option<Arc<Vec<f64>>>>,
    
    // Control Plane: Tracks the state of every node.
    pub status: Vec<u8>, 
    
    // Exception Plane: Sparse storage for errors.
    pub errors: HashMap<u32, ComputationError>,
    
    pub solver_trace: Option<Vec<SolverIteration>>,
}

impl Ledger {
    pub fn new() -> Self { Self::default() }

    pub fn ensure_capacity(&mut self, size: usize) {
        if self.status.len() < size {
            self.scalars.resize(size, 0.0);
            self.series.resize(size, None);
            self.status.resize(size, NodeStatus::Uncomputed as u8);
        }
    }

    // --- Fast Write API (Internal VM) ---

    #[inline(always)]
    pub fn set_scalar(&mut self, id: NodeId, val: f64) {
        let idx = id.index();
        self.scalars[idx] = val;
        self.status[idx] = NodeStatus::ComputedScalar as u8;
    }

    pub fn set_series(&mut self, id: NodeId, val: Arc<Vec<f64>>) {
        let idx = id.index();
        self.series[idx] = Some(val);
        self.status[idx] = NodeStatus::ComputedSeries as u8;
    }

    pub fn set_error(&mut self, id: NodeId, err: ComputationError) {
        let idx = id.index();
        self.status[idx] = NodeStatus::Error as u8;
        self.errors.insert(id.0, err);
    }
    
    // --- Compatibility API (Public / Legacy) ---

    pub fn insert(&mut self, id: NodeId, result: Result<Value, ComputationError>) {
        if id.index() >= self.status.len() {
             self.ensure_capacity(id.index() + 1);
        }
        match result {
            Ok(Value::Scalar(s)) => self.set_scalar(id, s),
            Ok(Value::Series(s)) => self.set_series(id, s),
            Err(e) => self.set_error(id, e),
        }
    }

    /// Reconstructs the `Option<Result<Value>>` view from the internal arrays.
    /// Note: This returns an OWNED Value, not a reference, because Value is constructed on the fly.
    pub fn get(&self, id: NodeId) -> Option<Result<Value, ComputationError>> {
        let idx = id.index();
        match self.status.get(idx).map(|&s| s)? {
            0 => None, // NodeStatus::Uncomputed
            1 => Some(Ok(Value::Scalar(self.scalars[idx]))), 
            2 => Some(Ok(Value::Series(self.series[idx].clone().unwrap()))),
            3 => Some(Err(self.errors.get(&id.0).cloned().unwrap_or(ComputationError::MathError("Unknown error".into())))),
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    pub fn is_computed(&self, id: NodeId) -> bool {
        if let Some(&s) = self.status.get(id.index()) {
            s != NodeStatus::Uncomputed as u8
        } else {
            false
        }
    }

    pub fn invalidate(&mut self, node_ids: impl IntoIterator<Item = NodeId>) {
        for id in node_ids {
            let idx = id.index();
            if idx < self.status.len() {
                self.status[idx] = NodeStatus::Uncomputed as u8;
            }
        }
    }
}