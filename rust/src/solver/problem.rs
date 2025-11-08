//! Defines the structure of a solver problem.
use crate::computation::{ComputationEngine, Ledger};
use crate::graph::{ComputationGraph, NodeId};

/// Describes the mathematical problem to be solved: a set of variables
/// and a set of equations that must equal zero.
pub struct PrismProblem<'a> {
    pub(crate) graph: &'a ComputationGraph,
    /// The node IDs of the variables the solver can change.
    pub(crate) variables: Vec<NodeId>,
    /// The node IDs of the residual formulas (LHS - RHS) for each constraint.
    pub(crate) residuals: Vec<NodeId>,
    /// A synchronous, single-threaded evaluator for use inside the solver loop.
    pub(crate) sync_engine: ComputationEngine<'a>,
    /// A template ledger containing all pre-computed, non-solver-dependent values.
    pub(crate) base_ledger: Ledger,
}