//! FFI bindings for the `graph` module, exposing its functionality to Python.

use crate::graph::{
    dag::ComputationGraph,
    edge::Edge,
    node::{Node, NodeId, NodeMetadata, Operation, TemporalType, Unit},
};
use crate::type_system::TypeChecker;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

#[pyclass(name = "_ComputationGraph")]
#[derive(Debug, Clone, Default)]
pub struct PyComputationGraph {
    graph: ComputationGraph,
}

// --- Rust-only helper methods ---
impl PyComputationGraph {
    /// Internal helper to add a binary formula node and its dependencies.
    fn add_binary_formula(
        &mut self,
        op: Operation,
        parents: Vec<usize>,
        name: String,
    ) -> usize {
        let parent_ids: Vec<NodeId> = parents.into_iter().map(NodeId::new).collect();
        let node = Node::Formula {
            op,
            parents: parent_ids.clone(),
            meta: NodeMetadata { name, ..Default::default() },
        };
        let child_id = self.graph.graph.add_node(node);
        for parent_id in parent_ids {
            self.graph.add_dependency(parent_id, child_id, Edge::Arithmetic);
        }
        child_id.index()
    }

    /// Parses an optional string from Python into a TemporalType.
    fn parse_temporal_type(temporal_type: Option<String>) -> PyResult<Option<TemporalType>> {
        match temporal_type.as_deref() {
            Some("Stock") => Ok(Some(TemporalType::Stock)),
            Some("Flow") => Ok(Some(TemporalType::Flow)),
            Some(other) => Err(PyValueError::new_err(format!(
                "Invalid temporal_type: '{}'",
                other
            ))),
            None => Ok(None),
        }
    }
}

// --- Python-exposed methods ---
#[pymethods]
impl PyComputationGraph {
    #[new]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_constant_node(
        &mut self,
        value: Vec<f64>,
        name: String,
        unit: Option<String>,
        temporal_type: Option<String>,
    ) -> PyResult<usize> {
        let meta = NodeMetadata {
            name,
            unit: unit.map(Unit),
            temporal_type: Self::parse_temporal_type(temporal_type)?,
        };
        let node_id = self.graph.add_constant(value, meta);
        Ok(node_id.index())
    }

    /// Updates metadata for a node and returns the *previous* metadata state.
    pub fn set_node_metadata(
        &mut self,
        node_id: usize,
        unit: Option<String>,
        temporal_type: Option<String>,
    ) -> PyResult<(Option<String>, Option<String>)> {
        let node_idx = NodeId::new(node_id);
        if let Some(node) = self.graph.graph.node_weight_mut(node_idx) {
            let meta = node.meta_mut();

            // Store old values before mutation.
            let old_unit = meta.unit.as_ref().map(|u| u.0.clone());
            let old_temporal_type = meta.temporal_type.as_ref().map(|tt| format!("{:?}", tt));
            
            // Apply new values if they are provided.
            if unit.is_some() {
                meta.unit = unit.map(Unit);
            }
            if temporal_type.is_some() {
                meta.temporal_type = Self::parse_temporal_type(temporal_type)?;
            }
            
            Ok((old_unit, old_temporal_type))
        } else {
            Err(PyValueError::new_err(format!("Node with id {} not found", node_id)))
        }
    }

    #[pyo3(name = "add_binary_formula")]
    pub fn py_add_binary_formula(
        &mut self,
        op_name: &str,
        parents: Vec<usize>,
        name: String,
    ) -> PyResult<usize> {
        let op = match op_name {
            "add" => Operation::Add,
            "subtract" => Operation::Subtract,
            "multiply" => Operation::Multiply,
            "divide" => Operation::Divide,
            _ => {
                return Err(PyValueError::new_err(format!(
                    "Unsupported operation: {}",
                    op_name
                )))
            }
        };
        Ok(self.add_binary_formula(op, parents, name))
    }

    pub fn add_formula_previous_value(
        &mut self,
        main_parent_idx: usize,
        default_parent_idx: usize,
        lag: u32,
        name: String,
    ) -> usize {
        let main_parent_id = NodeId::new(main_parent_idx);
        let default_parent_id = NodeId::new(default_parent_idx);

        let node = Node::Formula {
            op: Operation::PreviousValue {
                lag,
                default_node: default_parent_id,
            },
            parents: vec![main_parent_id, default_parent_id],
            meta: NodeMetadata {
                name,
                ..Default::default()
            },
        };
        let child_id = self.graph.graph.add_node(node);

        self.graph
            .add_dependency(main_parent_id, child_id, Edge::Temporal);
        self.graph
            .add_dependency(default_parent_id, child_id, Edge::DefaultValue);

        child_id.index()
    }

    #[pyo3(name = "validate")]
    pub fn py_validate(&self) -> PyResult<()> {
        let mut checker = TypeChecker::new(&self.graph);
        match checker.check_and_infer() {
            Ok(()) => Ok(()),
            Err(errors) => {
                // Collect and format all validation errors for a comprehensive report.
                let error_messages: Vec<String> = errors
                    .iter()
                    .map(|e| format!("Node '{}': {}", e.node_name, e.message))
                    .collect();

                Err(PyValueError::new_err(format!(
                    "Validation failed with {} error(s):\n- {}",
                    errors.len(),
                    error_messages.join("\n- ")
                )))
            }
        }
    }

    pub fn topological_order(&self) -> PyResult<Vec<usize>> {
        match self.graph.topological_order() {
            Ok(order) => Ok(order.into_iter().map(|id| id.index()).collect()),
            Err(cycle) => {
                let node_index = cycle.node_id().index();
                Err(PyValueError::new_err(format!(
                    "Graph contains a cycle: a node (id: {}) depends on itself.",
                    node_index
                )))
            }
        }
    }

    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }
}