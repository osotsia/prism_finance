//! FFI bindings for the `graph` module.

use crate::graph::{dag::ComputationGraph, edge::Edge, node::Node, node::NodeId, node::NodeMetadata, node::Operation};
use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;

/// A Python-accessible wrapper for the core Rust `ComputationGraph`.
///
/// This class is not intended for direct use by end-users. A higher-level,
/// more ergonomic API will be provided in the Python `prism_finance` package.
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

    /// Adds a `Constant` node to the graph.
    ///
    /// Returns the integer index of the new node.
    pub fn add_constant_node(&mut self, value: Vec<f64>, name: String) -> usize {
        let node = Node::Constant {
            value,
            meta: NodeMetadata { name, ..Default::default() },
        };
        self.graph.add_node(node).index()
    }

    /// Adds a `Formula` node to the graph for an addition operation.
    pub fn add_formula_add(&mut self, parents: Vec<usize>, name: String) -> usize {
        let parent_ids: Vec<NodeId> = parents.into_iter().map(NodeId::new).collect();
        let node = Node::Formula {
            op: Operation::Add,
            parents: parent_ids,
            meta: NodeMetadata { name, ..Default::default() },
        };
        self.graph.add_node(node).index()
    }

    /// Adds a dependency (an edge) between two nodes.
    pub fn add_dependency(&mut self, parent_idx: usize, child_idx: usize) {
        let parent_id = NodeId::new(parent_idx);
        let child_id = NodeId::new(child_idx);
        // For this basic example, all edges are considered Arithmetic.
        self.graph.add_dependency(parent_id, child_id, Edge::Arithmetic);
    }

    /// Returns the topological sort of the graph nodes.
    ///
    /// This provides a valid order of execution.
    ///
    /// Raises a `ValueError` if the graph contains a cycle.
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