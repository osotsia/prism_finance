use crate::store::NodeId;
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

/// A structure-of-arrays (SoA) backing store for model values.
/// Data is stored in a single contiguous block: [Node0_Data, Node1_Data, ...].
/// This ensures maximum cache locality and eliminates allocation during compute.
#[derive(Debug, Clone)]
pub struct Ledger {
    /// The primary memory block.
    /// Layout: Node 0 (len M), Node 1 (len M), ...
    data: Vec<f64>,
    
    /// The length of the time dimension (M).
    /// 1 for scalar models, N for time-series models.
    model_len: usize,

    /// Number of nodes capacity.
    node_capacity: usize,

    /// Optimization: Tracks if the Ledger has allocated memory.
    is_allocated: bool,
    
    pub solver_trace: Option<Vec<SolverIteration>>,
}

impl Ledger {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            model_len: 0,
            node_capacity: 0,
            is_allocated: false,
            solver_trace: None,
        }
    }

    /// Prepares the memory block.
    /// Must be called before any compute or IO.
    pub fn resize(&mut self, node_count: usize, model_len: usize) {
        if self.node_capacity != node_count || self.model_len != model_len {
            let total_size = node_count * model_len;
            self.data.resize(total_size, 0.0);
            self.model_len = model_len;
            self.node_capacity = node_count;
            self.is_allocated = true;
        }
    }

    /// Writes a value to a node's slot.
    /// Handles broadcasting: if input is scalar but model is vector, fills the slot.
    pub fn set_input(&mut self, node: NodeId, value: &[f64]) -> Result<(), ComputationError> {
        if !self.is_allocated {
            return Err(ComputationError::Mismatch { msg: "Ledger not allocated".into() });
        }
        
        let start = node.index() * self.model_len;
        let end = start + self.model_len;
        let dest = &mut self.data[start..end];

        if value.len() == 1 {
            // Broadcast scalar
            let v = value[0];
            for slot in dest.iter_mut() { *slot = v; }
        } else if value.len() == self.model_len {
            // Copy series
            dest.copy_from_slice(value);
        } else {
            return Err(ComputationError::Mismatch { 
                msg: format!("Input len {} != Model len {}", value.len(), self.model_len) 
            });
        }
        Ok(())
    }

    /// Reads a value. Always returns a slice of length `model_len`.
    pub fn get(&self, node: NodeId) -> Option<&[f64]> {
        if !self.is_allocated || node.index() >= self.node_capacity { return None; }
        
        let start = node.index() * self.model_len;
        Some(&self.data[start..start + self.model_len])
    }

    /// Returns the raw pointer to the data block.
    /// Used by the Engine for unsafe fast access.
    #[inline(always)]
    pub fn raw_data_mut(&mut self) -> *mut f64 {
        self.data.as_mut_ptr()
    }

    #[inline(always)]
    pub fn model_len(&self) -> usize { self.model_len }
}

impl Default for Ledger {
    fn default() -> Self { Self::new() }
}