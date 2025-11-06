//! Validation rule for temporal consistency (Stock vs. Flow).

use crate::graph::{Edge, Node, NodeId, TemporalType};
use crate::validation::error::{ValidationError, ValidationErrorType};
use petgraph::prelude::StableDiGraph;

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
    // This rule only applies to addition and subtraction formulas.
    let parents = match node {
        Node::Formula { op, parents, .. }
            if *op == crate::graph::Operation::Add || *op == crate::graph::Operation::Subtract =>
        {
            parents
        }
        _ => return None, // Not an Add/Subtract formula, so the rule doesn't apply.
    };

    // Find all parent nodes that are explicitly typed as 'Stock'.
    let stock_parents: Vec<&Node> = parents
        .iter()
        .filter_map(|id| graph.node_weight(*id)) // Get the parent Node struct.
        .filter(|p| p.meta().temporal_type == Some(TemporalType::Stock))
        .collect();

    // The core validation logic: if more than one parent is a stock, it's an error.
    if stock_parents.len() > 1 {
        // Collect the names for a clear error message.
        let stock_parent_names: Vec<String> = stock_parents
            .iter()
            .map(|p| p.meta().name.clone())
            .collect();

        return Some(ValidationError {
            node_id,
            error_type: ValidationErrorType::TemporalMismatch,
            message: format!(
                "Temporal Error: Attempted to add or subtract multiple 'Stock' variables ({:?}). Stocks represent points in time and cannot be summed directly.",
                stock_parent_names
            ),
        });
    }

    None
}