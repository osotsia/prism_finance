//! FFI bindings for the `graph` module, exposing its functionality to Python.

use crate::graph::{
    dag::ComputationGraph,
    edge::Edge,
    node::{Node, NodeId, NodeMetadata, Operation, TemporalType, Unit},
};
use crate::validation::Validator;
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
}

// --- Python-exposed methods ---
// This `impl` block IS marked with `#[pymethods]`.
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
        let parsed_temporal = match temporal_type.as_deref() {
            Some("Stock") => Some(TemporalType::Stock),
            Some("Flow") => Some(TemporalType::Flow),
            Some(other) => return Err(PyValueError::new_err(format!("Invalid temporal_type: '{}'", other))),
            None => None,
        };
        let meta = NodeMetadata {
            name,
            unit: unit.map(Unit),
            temporal_type: parsed_temporal,
        };
        let node_id = self.graph.add_constant(value, meta);
        Ok(node_id.index())
    }

    pub fn add_formula_add(&mut self, parents: Vec<usize>, name: String) -> usize {
        self.add_binary_formula(Operation::Add, parents, name)
    }

    pub fn add_formula_subtract(&mut self, parents: Vec<usize>, name: String) -> usize {
        self.add_binary_formula(Operation::Subtract, parents, name)
    }

    pub fn add_formula_multiply(&mut self, parents: Vec<usize>, name: String) -> usize {
        self.add_binary_formula(Operation::Multiply, parents, name)
    }

    pub fn add_formula_divide(&mut self, parents: Vec<usize>, name: String) -> usize {
        self.add_binary_formula(Operation::Divide, parents, name)
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
            op: Operation::PreviousValue { lag, default_node: default_parent_id },
            parents: vec![main_parent_id, default_parent_id],
            meta: NodeMetadata { name, ..Default::default() },
        };
        let child_id = self.graph.graph.add_node(node);

        self.graph.add_dependency(main_parent_id, child_id, Edge::Temporal);
        self.graph.add_dependency(default_parent_id, child_id, Edge::DefaultValue);
        
        child_id.index()
    }

    #[pyo3(name = "validate")]
    pub fn py_validate(&self) -> PyResult<()> {
        let validator = Validator::new(&self.graph);
        match validator.validate() {
            Ok(()) => Ok(()),
            Err(errors) => {
                let first_error = &errors[0];
                Err(PyValueError::new_err(format!(
                    "Validation failed at node {}: {}",
                    first_error.node_id.index(),
                    first_error.message
                )))
            }
        }
    }

    pub fn topological_order(&self) -> PyResult<Vec<usize>> {
        match self.graph.topological_order() {
            Ok(order) => Ok(order.into_iter().map(|id| id.index()).collect()),
            Err(cycle) => {
                let node_index = cycle.node_id().index();
                Err(PyValueError::new_err(format!("Graph contains a cycle: a node (id: {}) depends on itself.", node_index)))
            }
        }
    }

    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }
}