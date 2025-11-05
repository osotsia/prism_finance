//! Defines the error types for the validation module.
use crate::graph::NodeId;

/// The specific category of a validation error.
///
// This enum allows for programmatic inspection of errors, which is more
// robust than string matching on the error message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationErrorType {
    /// An error related to mismatched temporal types (e.g., adding two `Stock`s).
    TemporalMismatch,
    /// An error related to incompatible units (e.g., adding `USD` to `EUR`).
    UnitMismatch,
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