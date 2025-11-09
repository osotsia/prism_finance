//! Defines the structure of a solver problem.
use crate::computation::ledger::{Ledger, SolverIteration};
use crate::computation::ComputationEngine;
use crate::graph::{ComputationGraph, NodeId};
use std::sync::Mutex;

/// Describes the mathematical problem to be solved: a set of variables
/// and a set of equations that must equal zero.
pub struct PrismProblem<'a> {
    pub(crate) graph: &'a ComputationGraph,
    /// The node IDs of the variables the solver can change.
    pub(crate) variables: Vec<NodeId>,
    /// The node IDs of the residual formulas (LHS - RHS) for each constraint.
    pub(crate) residuals: Vec<NodeId>,
    /// The length of the time-series dimension of the model.
    pub(crate) model_len: usize,
    /// A synchronous, single-threaded evaluator for use inside the solver loop.
    pub(crate) sync_engine: ComputationEngine<'a>,
    /// A template ledger containing all pre-computed, non-solver-dependent values.
    pub(crate) base_ledger: Ledger,
    /// A thread-safe container to store iteration history from the C callback.
    pub(crate) iteration_history: Mutex<Vec<SolverIteration>>,
}