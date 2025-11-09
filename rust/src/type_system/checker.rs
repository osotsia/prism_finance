//! The central checker that executes all type system rules in the correct order.
use super::error::{ValidationError, ValidationErrorType};
use super::rules::{temporal, units};
use crate::graph::{ComputationGraph, Node, NodeId, NodeMetadata};
use std::collections::HashMap;

/// Orchestrates the static analysis and type inference process.
pub struct TypeChecker<'a> {
    graph: &'a ComputationGraph,
    inferred_meta: HashMap<NodeId, NodeMetadata>,
    errors: Vec<ValidationError>,
}

impl<'a> TypeChecker<'a> {
    pub fn new(graph: &'a ComputationGraph) -> Self {
        Self {
            graph,
            inferred_meta: HashMap::new(),
            errors: Vec::new(),
        }
    }

    /// Executes all rules against the graph, collecting errors, inferring types,
    /// and verifying user-declared types.
    pub fn check_and_infer(&mut self) -> Result<(), Vec<ValidationError>> {
        let order = match self.graph.topological_order() {
            Ok(order) => order,
            Err(cycle) => {
                self.errors.push(ValidationError::from_cycle(cycle));
                return Err(self.errors.clone());
            }
        };

        for node_id in order {
            if let Some(node) = self.graph.get_node(node_id) {
                self.check_node(node_id, node);
            }
        }

        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors.clone())
        }
    }

    /// The "Metaphorical Accountant": Checks a single node's "paperwork".
    /// 1.  **Gathers receipts**: Collects the already-verified types from parent nodes.
    /// 2.  **Calculates the total**: Infers the node's own type based on the operation.
    /// 3.  **Audits the calculation**: If the user declared an expected total (type),
    ///     it verifies the calculated total matches the expectation.
    /// 4.  **Files the result**: Stores the newly inferred type for downstream nodes to use.
    fn check_node(&mut self, node_id: NodeId, node: &Node) {
        // --- PHASE 1: INFERENCE ---
        // Infer the node's metadata based on its parents. For constants, this is trivial.
        // For formulas, it involves applying inference rules.
        let inferred_meta = match node {
            Node::Constant { meta, .. } => meta.clone(),
            Node::Formula { op, parents, .. } => {
                let parent_metas: Vec<NodeMetadata> = parents
                    .iter()
                    .map(|id| self.get_meta_for_node(*id).clone())
                    .collect();

                // Infer temporal type, pushing errors if inference fails.
                let temporal_type = match temporal::infer_and_validate(op, &parent_metas) {
                    Ok(t) => t,
                    Err(e) => {
                        self.errors.push(e.at_node(node_id, node.meta().name.clone()));
                        None
                    }
                };
                // Infer unit, pushing errors if inference fails.
                let unit = match units::infer_and_validate(op, &parent_metas) {
                    Ok(u) => u,
                    Err(e) => {
                        self.errors.push(e.at_node(node_id, node.meta().name.clone()));
                        None
                    }
                };

                NodeMetadata {
                    name: node.meta().name.clone(),
                    temporal_type,
                    unit,
                }
            }
            Node::SolverVariable { meta, .. } => meta.clone(), 
        };

        // --- PHASE 2: VERIFICATION ---
        // If the node is a formula with user-declared types, verify they match the
        // inferred types from Phase 1.
        if let Node::Formula { meta: declared_meta, .. } = node {
            // Verify temporal type
            if let Some(declared_tt) = &declared_meta.temporal_type {
                if Some(declared_tt) != inferred_meta.temporal_type.as_ref() {
                    let inferred_str = inferred_meta
                        .temporal_type
                        .as_ref()
                        .map_or("None", |t| match t {
                            crate::graph::TemporalType::Stock => "Stock",
                            crate::graph::TemporalType::Flow => "Flow",
                        });

                    let msg = format!(
                        "Verification Error: Declared temporal type '{:?}' does not match inferred type '{}'.",
                        declared_tt, inferred_str
                    );
                    self.errors.push(ValidationError {
                        node_id,
                        node_name: node.meta().name.clone(),
                        error_type: ValidationErrorType::TemporalMismatch,
                        message: msg,
                    });
                }
            }
            // Verify unit
            if let Some(declared_u) = &declared_meta.unit {
                if Some(declared_u) != inferred_meta.unit.as_ref() {
                    let inferred_str = inferred_meta.unit.as_ref().map_or("None", |u| u.0.as_str());
                    let msg = format!(
                        "Verification Error: Declared unit '{}' does not match inferred unit '{}'.",
                        declared_u.0,
                        inferred_str
                    );
                     self.errors.push(ValidationError {
                        node_id,
                        node_name: node.meta().name.clone(),
                        error_type: ValidationErrorType::UnitMismatch,
                        message: msg,
                    });
                }
            }
        }
        
        // --- PHASE 3: STORAGE ---
        // Store the *inferred* metadata. This is crucial, as downstream nodes
        // must build upon the inferred reality, not the user's declaration.
        self.inferred_meta.insert(node_id, inferred_meta);
    }

    /// Gets metadata for a node, preferring newly inferred metadata over original.
    fn get_meta_for_node(&self, node_id: NodeId) -> &NodeMetadata {
        // The `inferred_meta` map contains the results of previous steps in the
        // topological sort, so we can rely on it being populated for any parent node.
        self.inferred_meta.get(&node_id)
            .expect("BUG: Parent node metadata not found during check. This indicates a non-topological traversal.")
    }
}