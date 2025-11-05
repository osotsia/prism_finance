//! Validation rule for temporal consistency (Stock vs. Flow).

use crate::graph::{Node, NodeId, NodeMetadata, Operation, TemporalType};
use crate::validation::error::{ValidationError, ValidationErrorType};
use petgraph::prelude::StableDiGraph;
use crate::graph::Edge;

/// "The Accountant's Ledger Rule": Ensures that stocks and flows are combined logically.
///
/// This rule prevents fundamental accounting errors. For V1, it enforces a simple,
/// high-impact constraint: you cannot add or subtract two `Stock` types. This prevents
/// nonsensical operations like `Opening Balance + Closing Balance`. The valid pattern,
/// `Stock[t] = Stock[t-1] + Flow[t]`, is handled by the `PreviousValue` operation,
/// not simple addition.
pub(crate) fn validate_temporal(
    graph: &StableDiGraph<Node, Edge>,
    node_id: NodeId,
    node: &Node,
) -> Option<ValidationError> {
    if let Node::Formula { op: Operation::Add | Operation::Subtract, parents, .. } = node {
        // Collect the temporal types of all parent nodes.
        let parent_types: Vec<Option<&TemporalType>> = parents
            .iter()
            .filter_map(|&p_id| graph.node_weight(p_id))
            .map(|p_node| match p_node {
                Node::Constant { meta, .. } => meta.temporal_type.as_ref(),
                Node::Formula { meta, .. } => meta.temporal_type.as_ref(),
                Node::SolverVariable { meta } => meta.temporal_type.as_ref(),
            })
            .collect();

        // Check for the invalid "Stock + Stock" or "Stock - Stock" operation.
        let stock_count = parent_types.iter().filter(|&&t| t == Some(&TemporalType::Stock)).count();

        if stock_count > 1 {
            let parent_names: Vec<String> = parents
                .iter()
                .filter_map(|&p_id| graph.node_weight(p_id))
                .map(|p_node| match p_node {
                     Node::Constant { meta, .. } => meta.name.clone(),
                     Node::Formula { meta, .. } => meta.name.clone(),
                     Node::SolverVariable { meta } => meta.name.clone(),
                })
                .collect();
            
            return Some(ValidationError {
                node_id,
                error_type: ValidationErrorType::TemporalMismatch,
                message: format!(
                    "Temporal Error: Attempted to add or subtract multiple 'Stock' variables ({:?}). Stocks represent points in time and cannot be summed directly.",
                    parent_names
                ),
            });
        }
    }
    None
}