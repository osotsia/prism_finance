//! Defines the error types for the type system module.
use crate::graph::NodeId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationErrorType {
    TemporalMismatch,
    UnitMismatch,
    Structural,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub node_id: NodeId,
    pub node_name: String,
    pub error_type: ValidationErrorType,
    pub message: String,
}

impl ValidationError {
    pub fn from_string(msg: String) -> Self {
        Self {
            node_id: NodeId(0), // Placeholder
            node_name: "Graph".to_string(),
            error_type: ValidationErrorType::Structural,
            message: msg,
        }
    }
    
    pub fn at_node(mut self, node_id: NodeId, node_name: String) -> Self {
        self.node_id = node_id;
        self.node_name = node_name;
        self
    }
}