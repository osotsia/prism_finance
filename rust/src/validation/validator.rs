//! The central validator that orchestrates the execution of all validation rules.
use super::error::ValidationError;
use super::rules::{temporal, units};
use crate::graph::ComputationGraph;

/// The orchestrator for the static analysis engine.
///
/// This struct holds a reference to the graph and iterates through its nodes,
/// applying a set of validation rules to each one. It's like a code linter,
/// collecting all potential errors before "compilation" (i.e., calculation).
pub struct Validator<'a> {
    graph: &'a ComputationGraph,
}

impl<'a> Validator<'a> {
    /// Creates a new validator for the given computation graph.
    pub fn new(graph: &'a ComputationGraph) -> Self {
        Self { graph }
    }

    /// Executes all registered validation rules against the graph.
    ///
    /// # Returns
    /// - `Ok(())` if no validation errors are found.
    /// - `Err(Vec<ValidationError>)` containing all errors discovered in the graph.
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();
        
        // We don't need a topological sort here, as validation rules are local
        // to a node and its direct parents. Iterating all nodes is sufficient.
        for node_id in self.graph.graph.node_indices() {
            if let Some(node) = self.graph.graph.node_weight(node_id) {
                // Run the temporal consistency rule.
                if let Some(err) = temporal::validate_temporal(&self.graph.graph, node_id, node) {
                    errors.push(err);
                }

                // Run the unit consistency rule.
                if let Some(err) = units::validate_units(&self.graph.graph, node_id, node) {
                    errors.push(err);
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}