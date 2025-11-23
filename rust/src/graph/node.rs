//! Defines the `Node` and its associated types.

// Changed: Import NodeId from storage, remove petgraph dependency
use crate::graph::storage::NodeId; 

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TemporalType {
    Stock,
    Flow,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Unit(pub String);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeMetadata {
    pub name: String,
    pub temporal_type: Option<TemporalType>,
    pub unit: Option<Unit>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operation {
    Add,
    Subtract,
    Multiply,
    Divide,
    /// Represents a time-series lag operation (e.g., `.prev()`).
    /// The `default_node` provides the value for initial periods.
    PreviousValue { lag: u32, default_node: NodeId },
}

/// The primary enum representing a node in the computation graph.
/// Note: With the new Registry architecture, this Enum is mostly for
/// backward compatibility or external views, as the engine uses `NodeKind`.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Constant { meta: NodeMetadata },
    Formula {
        op: Operation,
        parents: Vec<NodeId>,
        meta: NodeMetadata,
    },
    SolverVariable { 
        meta: NodeMetadata,
        is_temporal_dependency: bool,
    },
}

impl Node {
    pub fn meta(&self) -> &NodeMetadata {
        match self {
            Node::Constant { meta, .. } => meta,
            Node::Formula { meta, .. } => meta,
            Node::SolverVariable { meta, .. } => meta,
        }
    }
}