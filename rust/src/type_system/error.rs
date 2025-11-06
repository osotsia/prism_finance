//! Defines the error types for the type system module.
use crate::graph::NodeId;
use petgraph::algo::Cycle;

/// The specific category of a validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationErrorType {
    /// An error related to mismatched temporal types (e.g., adding two `Stock`s).
    TemporalMismatch,
    /// An error related to incompatible units (e.g., adding `USD` to `EUR`).
    UnitMismatch,
    /// A structural error in the graph itself, such as a cycle.
    Structural,
}

/// A structured error report from the static analysis engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// The ID of the node where the error was detected.
    pub node_id: NodeId,
    /// The category of the error.
    pub error_type: ValidationErrorType,
    /// A human-readable message explaining the error.
    pub message: String,
}

impl ValidationError {
    /// Helper to create a validation error from a graph cycle.
    pub fn from_cycle(cycle: Cycle<NodeId>) -> Self {
        let node_id = cycle.node_id();
        Self {
            node_id,
            error_type: ValidationErrorType::Structural,
            message: format!(
                "Structural Error: Graph contains a cycle. Node {} depends on itself.",
                node_id.index()
            ),
        }
    }
}