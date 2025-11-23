//! Defines the core data structures for the computation graph.
pub mod dag;
pub mod storage; // New module
pub mod edge;    // Kept for Edge enum types (Temporal/Arithmetic), though mostly implicit now.
pub mod node;

// Re-export key types for convenient access
pub use dag::ComputationGraph;
pub use storage::{NodeId, NodeKind}; // Export NodeKind and the new NodeId
pub use node::{NodeMetadata, Operation, TemporalType, Unit};