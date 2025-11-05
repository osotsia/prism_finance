//! Validation rule for dimensional analysis (Units).

use crate::graph::{Node, NodeId, Operation, Unit};
use crate::validation::error::{ValidationError, ValidationErrorType};
use petgraph::prelude::StableDiGraph;
use crate::graph::Edge;
use std::collections::HashSet;

/// "The Apples and Oranges Rule": Ensures that you only add or subtract like quantities.
///
/// For V1, this rule enforces that for addition or subtraction, all parent nodes
/// with a defined unit must have the *exact same* unit. This prevents errors like
/// `Revenue (USD) + Volume (MWh)`. More complex rules for multiplication/division
/// can be added later.
pub(crate) fn validate_units(
    graph: &StableDiGraph<Node, Edge>,
    node_id: NodeId,
    node: &Node,
) -> Option<ValidationError> {
    if let Node::Formula { op: Operation::Add | Operation::Subtract, parents, .. } = node {
        // Collect all unique, defined units from parent nodes.
        let unique_units: HashSet<Unit> = parents
            .iter()
            .filter_map(|&p_id| graph.node_weight(p_id))
            .filter_map(|p_node| match p_node {
                Node::Constant { meta, .. } => meta.unit.clone(),
                Node::Formula { meta, .. } => meta.unit.clone(),
                Node::SolverVariable { meta } => meta.unit.clone(),
            })
            .collect();

        // If there is more than one unique unit, it's an error.
        if unique_units.len() > 1 {
            let units_str: Vec<String> = unique_units.into_iter().map(|u| u.0).collect();
            return Some(ValidationError {
                node_id,
                error_type: ValidationErrorType::UnitMismatch,
                message: format!(
                    "Unit Mismatch: Attempted to add or subtract variables with incompatible units: {:?}. All units must be identical for this operation.",
                    units_str
                ),
            });
        }
    }
    None
}