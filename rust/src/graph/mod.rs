//! Defines the core data structures for the computation graph.
//!
//! The graph is a Directed Acyclic Graph (DAG) where nodes represent
//! financial variables (constants, formulas) and edges represent
//! dependencies between them. This module provides the building blocks
//! for constructing, manipulating, and analyzing the model's structure.

pub mod dag;
pub mod edge;
pub mod node;

// Re-export key types for convenient access from other modules.
pub use dag::ComputationGraph;
pub use edge::Edge;
pub use node::{Node, NodeId, NodeMetadata, Operation, TemporalType, Unit};