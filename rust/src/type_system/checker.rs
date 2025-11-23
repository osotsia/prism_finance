//! The central checker that executes all type system rules.
use super::error::{ValidationError, ValidationErrorType};
use super::rules::{temporal, units};
use crate::graph::{ComputationGraph, NodeId, NodeKind, NodeMetadata};
use std::collections::HashMap;

pub struct TypeChecker<'a> {
    graph: &'a ComputationGraph,
    inferred_meta: HashMap<NodeId, NodeMetadata>,
    errors: Vec<ValidationError>,
}

impl<'a> TypeChecker<'a> {
    pub fn new(graph: &'a ComputationGraph) -> Self {
        Self { graph, inferred_meta: HashMap::new(), errors: Vec::new() }
    }

    pub fn check_and_infer(&mut self) -> Result<(), Vec<ValidationError>> {
        let order = match self.graph.topological_order() {
            Ok(o) => o,
            Err(msg) => {
                self.errors.push(ValidationError::from_string(msg));
                return Err(self.errors.clone());
            }
        };

        for node_id in order {
            self.check_node(node_id);
        }

        if self.errors.is_empty() { Ok(()) } else { Err(self.errors.clone()) }
    }

    fn check_node(&mut self, node_id: NodeId) {
        let kind = self.graph.get_node_kind(node_id);
        let meta = self.graph.get_node_meta(node_id);

        // --- PHASE 1: INFERENCE ---
        let inferred_meta = match kind {
            NodeKind::Scalar(_) | NodeKind::TimeSeries(_) | NodeKind::SolverVariable => meta.clone(),
            NodeKind::Formula(op) => {
                let parents = self.graph.get_parents(node_id);
                let parent_metas: Vec<NodeMetadata> = parents.iter()
                    .map(|id| self.inferred_meta.get(id).cloned().unwrap_or_default())
                    .collect();

                let temporal_type = match temporal::infer_and_validate(op, &parent_metas) {
                    Ok(t) => t,
                    Err(e) => { self.errors.push(e.at_node(node_id, meta.name.clone())); None }
                };
                let unit = match units::infer_and_validate(op, &parent_metas) {
                    Ok(u) => u,
                    Err(e) => { self.errors.push(e.at_node(node_id, meta.name.clone())); None }
                };
                NodeMetadata { name: meta.name.clone(), temporal_type, unit }
            }
        };

        // --- PHASE 2: VERIFICATION ---
        if let NodeKind::Formula(_) = kind {
            // Verify Temporal Type
            if let Some(decl) = &meta.temporal_type {
                if Some(decl) != inferred_meta.temporal_type.as_ref() {
                    let decl_str = format!("{:?}", decl); // e.g. "Stock"
                    let inf_str = inferred_meta.temporal_type.as_ref()
                        .map(|t| format!("{:?}", t))
                        .unwrap_or_else(|| "None".to_string());
                    
                    self.errors.push(ValidationError {
                        node_id, node_name: meta.name.clone(),
                        error_type: ValidationErrorType::TemporalMismatch,
                        message: format!("Declared temporal type '{}' does not match inferred type '{}'", decl_str, inf_str)
                    });
                }
            }
            
            // Verify Unit
            if let Some(decl) = &meta.unit {
                if Some(decl) != inferred_meta.unit.as_ref() {
                     let decl_str = &decl.0; // e.g. "USD"
                     let inf_str = inferred_meta.unit.as_ref()
                        .map(|u| u.0.as_str())
                        .unwrap_or("None");

                     self.errors.push(ValidationError {
                        node_id, node_name: meta.name.clone(),
                        error_type: ValidationErrorType::UnitMismatch,
                        message: format!("Declared unit '{}' does not match inferred unit '{}'", decl_str, inf_str)
                    });
                }
            }
        }

        self.inferred_meta.insert(node_id, inferred_meta);
    }
}