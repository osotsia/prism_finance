//! Validation rule for dimensional analysis (Units).

use crate::graph::{Edge, Node, NodeId, Unit};
use crate::validation::error::{ValidationError, ValidationErrorType};
use petgraph::prelude::StableDiGraph;
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
    // This rule only applies to addition and subtraction formulas.
    let parents = match node {
        Node::Formula { op, parents, .. }
            if *op == crate::graph::Operation::Add || *op == crate::graph::Operation::Subtract =>
        {
            parents
        }
        _ => return None, // Not an Add/Subtract formula, so the rule doesn't apply.
    };

    // Collect all unique, defined units from parent nodes into a HashSet.
    // The use of the `meta()` helper makes this extremely concise.
    let unique_units: HashSet<&Unit> = parents
        .iter()
        .filter_map(|id| graph.node_weight(*id)) // Get parent Node.
        .filter_map(|p| p.meta().unit.as_ref()) // Get an Option<&Unit> and unwrap.
        .collect();

    // The core validation logic: if there is more than one unique unit, it's an error.
    if unique_units.len() > 1 {
        let units_str: Vec<String> = unique_units.into_iter().map(|u| u.0.clone()).collect();
        return Some(ValidationError {
            node_id,
            error_type: ValidationErrorType::UnitMismatch,
            message: format!(
                "Unit Mismatch: Attempted to add or subtract variables with incompatible units: {:?}. All units must be identical for this operation.",
                units_str
            ),
        });
    }

    None
}