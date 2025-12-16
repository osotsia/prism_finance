use crate::store::{Registry, NodeId}; // NodeId kept for traits if needed, but not for Vecs
use crate::compute::ledger::{Ledger, SolverIteration};
use crate::compute::bytecode::Program;
use std::sync::Mutex;

pub struct PrismProblem<'a> {
    pub registry: &'a Registry,
    pub program: &'a Program, 
    
    // Updated to usize to match physical ledger addressing
    pub variables: Vec<usize>,
    pub residuals: Vec<usize>,
    
    pub model_len: usize,
    pub base_ledger: Ledger,
    pub iteration_history: Mutex<Vec<SolverIteration>>,
}