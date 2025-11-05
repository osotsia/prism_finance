//! FFI bindings for the `graph` module, exposing its functionality to Python.

// --- Crate-level Imports ---
use crate::graph::{
    dag::ComputationGraph,
    edge::Edge,
    node::{Node, NodeId, NodeMetadata, Operation, TemporalType, Unit},
};
use crate::validation::Validator;

// --- Third-party Imports ---
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// A Python-accessible wrapper for the core Rust `ComputationGraph`.
///
/// This class is not intended for direct use by end-users. A higher-level,
/// more ergonomic API is provided in the Python `prism_finance` package.
/// The leading underscore in the name `_ComputationGraph` signals this intent.
#[pyclass(name = "_ComputationGraph")]
#[derive(Debug, Clone, Default)]
pub struct PyComputationGraph {
    graph: ComputationGraph,
}

#[pymethods]
impl PyComputationGraph {
    /// Creates a new, empty computation graph.
    #[new]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a `Constant` node to the graph with optional metadata.
    ///
    /// This is the primary entry point for adding input variables from Python.
    ///
    /// Returns the integer index of the new node.
    ///
    /// # Arguments
    /// * `value` - A vector of f64 representing the time-series data.
    /// * `name` - A human-readable name for the node.
    /// * `unit` - An optional string (e.g., "USD") for unit validation.
    /// * `temporal_type` - An optional string ("Stock" or "Flow") for temporal validation.
    ///
    /// # Raises
    /// * `ValueError` if `temporal_type` is not "Stock", "Flow", or None.
    pub fn add_constant_node(
        &mut self,
        value: Vec<f64>,
        name: String,
        unit: Option<String>,
        temporal_type: Option<String>,
    ) -> PyResult<usize> {
        // Parse the optional temporal_type string from Python into the Rust enum.
        let parsed_temporal = match temporal_type.as_deref() {
            Some("Stock") => Some(TemporalType::Stock),
            Some("Flow") => Some(TemporalType::Flow),
            Some(other) => {
                return Err(PyValueError::new_err(format!(
                    "Invalid temporal_type: '{}'. Must be 'Stock' or 'Flow'.",
                    other
                )))
            }
            None => None,
        };

        let node = Node::Constant {
            value,
            meta: NodeMetadata {
                name,
                unit: unit.map(Unit), // Convert Option<String> to Option<Unit>
                temporal_type: parsed_temporal,
            },
        };

        Ok(self.graph.add_node(node).index())
    }

    /// Adds a `Formula` node to the graph for an addition operation.
    ///
    /// # Arguments
    /// * `parents` - A vector of node indices that are inputs to this formula.
    /// * `name` - A human-readable name for the resulting node.
    pub fn add_formula_add(&mut self, parents: Vec<usize>, name: String) -> usize {
        let parent_ids: Vec<NodeId> = parents.into_iter().map(NodeId::new).collect();
        let node = Node::Formula {
            op: Operation::Add,
            parents: parent_ids,
            meta: NodeMetadata {
                name,
                ..Default::default()
            },
        };
        self.graph.add_node(node).index()
    }

    /// Adds a dependency (an edge) between two nodes.
    ///
    /// # Arguments
    /// * `parent_idx` - The index of the node that provides data.
    /// * `child_idx` - The index of the node that consumes data.
    pub fn add_dependency(&mut self, parent_idx: usize, child_idx: usize) {
        let parent_id = NodeId::new(parent_idx);
        let child_id = NodeId::new(child_idx);
        // For this basic example, all edges are considered Arithmetic.
        // This will be expanded as more operation types are added.
        self.graph
            .add_dependency(parent_id, child_id, Edge::Arithmetic);
    }

    /// Performs static analysis on the graph, checking for logical errors.
    ///
    /// This method runs a suite of validation rules (e.g., temporal and unit
    /// consistency).
    ///
    /// # Raises
    /// * `ValueError` with a descriptive message if any validation rule fails.
    #[pyo3(name = "validate")]
    pub fn py_validate(&self) -> PyResult<()> {
        let validator = Validator::new(&self.graph);
        match validator.validate() {
            Ok(()) => Ok(()),
            Err(errors) => {
                // For a clear user experience, raise an exception for the first error found.
                let first_error = &errors[0];
                Err(PyValueError::new_err(format!(
                    "Validation failed at node {}: {}",
                    first_error.node_id.index(),
                    first_error.message
                )))
            }
        }
    }

    /// Returns the topological sort of the graph nodes, defining a valid execution order.
    ///
    /// # Raises
    /// * `ValueError` if the graph contains a cycle.
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

    /// Returns the total number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }
}