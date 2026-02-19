use thiserror::Error;
use serde::{Serialize, Deserialize};

#[derive(Error, Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolverIteration {
    pub iter_count: i32,
    pub obj_value: f64,
    pub inf_pr: f64,
    pub inf_du: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ledger {
    data: Vec<f64>,
    model_len: usize,
    capacity: usize,
    is_allocated: bool,
    pub solver_trace: Option<Vec<SolverIteration>>,
}

impl Ledger {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            model_len: 0,
            capacity: 0,
            is_allocated: false,
            solver_trace: None,
        }
    }

    pub fn resize(&mut self, node_count: usize, model_len: usize) {
        if self.capacity != node_count || self.model_len != model_len {
            let total_size = node_count * model_len;
            self.data.resize(total_size, 0.0);
            self.model_len = model_len;
            self.capacity = node_count;
            self.is_allocated = true;
        }
    }

    /// Writes data to a physical storage index.
    pub fn set_input_at_index(&mut self, index: usize, value: &[f64]) -> Result<(), ComputationError> {
        if !self.is_allocated {
            return Err(ComputationError::Mismatch { msg: "Ledger not allocated".into() });
        }
        
        let start = index * self.model_len;
        let end = start + self.model_len;
        
        if end > self.data.len() {
             return Err(ComputationError::Mismatch { msg: "Index out of bounds".into() });
        }
        
        let dest = &mut self.data[start..end];

        if value.len() == 1 {
            let v = value[0];
            for slot in dest.iter_mut() { *slot = v; }
        } else if value.len() == self.model_len {
            dest.copy_from_slice(value);
        } else {
            return Err(ComputationError::Mismatch { 
                msg: format!("Input len {} != Model len {}", value.len(), self.model_len) 
            });
        }
        Ok(())
    }

    /// Reads data from a physical storage index.
    pub fn get_at_index(&self, index: usize) -> Option<&[f64]> {
        if !self.is_allocated || index >= self.capacity { return None; }
        let start = index * self.model_len;
        Some(&self.data[start..start + self.model_len])
    }

    #[inline(always)]
    pub fn raw_data_mut(&mut self) -> *mut f64 {
        self.data.as_mut_ptr()
    }

    #[inline(always)]
    pub fn raw_data_len(&self) -> usize {
        self.data.len()
    }

    #[inline(always)]
    pub fn model_len(&self) -> usize { self.model_len }

    /// Helper to access the underlying vector mutably. 
    /// Useful for advanced manipulation or testing internal states.
    #[inline(always)]
    pub fn raw_data_mut_vec(&mut self) -> &mut Vec<f64> {
        &mut self.data
    }
}

impl Default for Ledger {
    fn default() -> Self { Self::new() }
}