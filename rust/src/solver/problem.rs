use crate::store::{Registry, NodeId};
use crate::compute::engine::Engine;
use crate::compute::ledger::{Ledger, SolverIteration};
use std::sync::Mutex;

pub struct PrismProblem<'a> {
    pub registry: &'a Registry,
    pub engine: Engine<'a>,
    
    pub variables: Vec<NodeId>,
    pub residuals: Vec<NodeId>,
    pub model_len: usize,
    
    pub base_ledger: Ledger,
    pub iteration_history: Mutex<Vec<SolverIteration>>,
}