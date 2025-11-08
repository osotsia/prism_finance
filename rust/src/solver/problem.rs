//! Defines the structure of a solver problem.
/*
use crate::graph::{ComputationGraph, NodeId};
use crate::computation::{ComputationEngine, Ledger};

/// Describes the mathematical problem to be solved: a set of variables
/// and a set of equations that must equal zero.
pub struct SolverProblem<'a> {
    pub(crate) graph: &'a ComputationGraph,
    /// The node IDs of the variables the solver can change.
    pub(crate) variables: Vec<NodeId>,
    /// The node IDs of the constraint equations to be evaluated.
    pub(crate) constraints: Vec<NodeId>,
    /// A synchronous, single-threaded evaluator for use inside the solver loop.
    pub(crate) sync_engine: ComputationEngine<'a>,
    /// A template ledger containing all non-solver-dependent values.
    pub(crate) base_ledger: Ledger,
}
*/