//! The central checker that executes all type system rules in the correct order.
use super::error::ValidationError;
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

    /// Executes all rules against the graph, collecting errors and inferring types.
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

    /// Checks a single node, inferring its metadata or adding an error.
    fn check_node(&mut self, node_id: NodeId, node: &Node) {
        let inferred_meta = match node {
            Node::Constant { meta, .. } => meta.clone(),
            Node::Formula { op, parents, meta } => {
                // --- FIX APPLIED HERE ---
                // Collect owned metadata by cloning. This ends the immutable borrow of `self`
                // after this line, allowing `self.errors` to be borrowed mutably later.
                let parent_metas: Vec<NodeMetadata> = parents
                    .iter()
                    .map(|id| self.get_meta_for_node(*id).clone())
                    .collect();

                let temporal_type =
                    match temporal::infer_and_validate(op, &parent_metas) {
                        Ok(t) => t,
                        Err(mut e) => {
                            e.node_id = node_id;
                            self.errors.push(e);
                            None
                        }
                    };

                let unit = match units::infer_and_validate(op, &parent_metas) {
                    Ok(u) => u,
                    Err(mut e) => {
                        e.node_id = node_id;
                        self.errors.push(e);
                        None
                    }
                };

                NodeMetadata {
                    name: meta.name.clone(),
                    temporal_type,
                    unit,
                }
            }
            Node::SolverVariable { meta } => meta.clone(),
        };

        self.inferred_meta.insert(node_id, inferred_meta);
    }

    /// Gets metadata for a node, preferring newly inferred metadata over original.
    fn get_meta_for_node(&self, node_id: NodeId) -> &NodeMetadata {
        if let Some(meta) = self.inferred_meta.get(&node_id) {
            return meta;
        }
        self.graph.get_node(node_id).unwrap().meta()
    }
}