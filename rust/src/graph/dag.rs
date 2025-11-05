//! Implements the `ComputationGraph`, the primary data structure for storing the model's logic.

use super::edge::Edge;
use super::node::{Node, NodeId};
use petgraph::{algo::toposort, stable_graph::StableDiGraph};

/// A computation graph representing the financial model.
///
/// This structure acts as a "blueprint" of the model. It contains all the
/// variables and formulas, but not their computed results. It is a wrapper
// around `petgraph::StableDiGraph` to provide a domain-specific API and
// ensure that node indices remain stable even after removals.
#[derive(Debug, Clone, Default)]
pub struct ComputationGraph {
    pub(crate) graph: StableDiGraph<Node, Edge>,
}

impl ComputationGraph {
    /// Creates a new, empty computation graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a new node to the graph and returns its unique, stable ID.
    pub fn add_node(&mut self, node: Node) -> NodeId {
        self.graph.add_node(node)
    }

    /// Adds a directed dependency (an edge) from a parent node to a child node.
    ///
    /// This signifies that the `child` node's calculation depends on the `parent` node's value.
    pub fn add_dependency(&mut self, parent: NodeId, child: NodeId, edge_type: Edge) {
        self.graph.add_edge(parent, child, edge_type);
    }

    /// Retrieves a reference to a node's data by its ID.
    pub fn get_node(&self, id: NodeId) -> Option<&Node> {
        self.graph.node_weight(id)
    }

    /// Returns a vector of node IDs in a valid evaluation order (topological sort).
    ///
    /// This is the "critical path" for the `computation::Evaluator`. The evaluator
    /// will iterate through this list to compute the model, ensuring that all
    /// dependencies are calculated before the nodes that need them.
    ///
    /// # Returns
    /// - `Ok(Vec<NodeId>)` on success.
    /// - `Err` if the graph contains a cycle, which indicates a logical error
    ///   in the model's non-solver construction.
    pub fn topological_order(&self) -> Result<Vec<NodeId>, petgraph::algo::Cycle<NodeId>> {
        toposort(&self.graph, None)
    }

    /// Returns the total number of nodes currently in the graph.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }
}

// Co-located unit tests for the ComputationGraph.
// This allows testing of the module's internal logic and API without
// needing to build the entire application.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::node::{NodeMetadata, Operation};

    /// Test case: Verifies that a simple, valid DAG produces the correct topological order.
    ///
    /// Graph structure:
    ///   A (const) -> C (formula)
    ///   B (const) -> C (formula)
    #[test]
    fn topological_sort_on_valid_dag_succeeds() {
        let mut graph = ComputationGraph::new();

        // Define nodes
        let node_a = graph.add_node(Node::Constant {
            value: vec![10.0],
            meta: NodeMetadata::default(),
        });
        let node_b = graph.add_node(Node::Constant {
            value: vec![20.0],
            meta: NodeMetadata::default(),
        });
        let node_c = graph.add_node(Node::Formula {
            op: Operation::Add,
            parents: vec![node_a, node_b],
            meta: NodeMetadata::default(),
        });

        // Define dependencies
        graph.add_dependency(node_a, node_c, Edge::Arithmetic);
        graph.add_dependency(node_b, node_c, Edge::Arithmetic);

        let order = graph
            .topological_order()
            .expect("Topological sort should succeed for a valid DAG");

        // Assert that the order is correct. A and B can appear in any order,
        // but C must be last.
        assert_eq!(order.len(), 3, "Order should contain all three nodes");
        let pos_c = order.iter().position(|&id| id == node_c).unwrap();
        let pos_a = order.iter().position(|&id| id == node_a).unwrap();
        let pos_b = order.iter().position(|&id| id == node_b).unwrap();

        assert!(pos_c > pos_a, "Node C must be evaluated after Node A");
        assert!(pos_c > pos_b, "Node C must be evaluated after Node B");
    }

    /// Test case: Verifies that cycle detection works correctly.
    ///
    /// Graph structure:
    ///   A -> B -> C -> A (a direct cycle)
    #[test]
    fn topological_sort_on_cyclic_graph_fails() {
        let mut graph = ComputationGraph::new();

        // Define nodes
        let node_a = graph.add_node(Node::SolverVariable {
            meta: NodeMetadata::default(),
        });
        let node_b = graph.add_node(Node::SolverVariable {
            meta: NodeMetadata::default(),
        });
        let node_c = graph.add_node(Node::SolverVariable {
            meta: NodeMetadata::default(),
        });

        // Define dependencies that form a cycle
        graph.add_dependency(node_a, node_b, Edge::Arithmetic);
        graph.add_dependency(node_b, node_c, Edge::Arithmetic);
        graph.add_dependency(node_c, node_a, Edge::Arithmetic); // This edge creates the cycle

        let result = graph.topological_order();

        assert!(
            result.is_err(),
            "Topological sort must fail for a cyclic graph"
        );
    }

    /// Test case: Verifies adding and retrieving nodes by ID.
    #[test]
    fn add_and_get_node() {
        let mut graph = ComputationGraph::new();
        let meta = NodeMetadata {
            name: "Test Node".to_string(),
            ..Default::default()
        };
        let node_data = Node::Constant {
            value: vec![1.0],
            meta: meta.clone(),
        };

        let node_id = graph.add_node(node_data.clone());
        let retrieved_node = graph.get_node(node_id).expect("Node should be retrievable");

        // Using PartialEq on Node enum to verify correctness
        assert_eq!(
            *retrieved_node, node_data,
            "Retrieved node data does not match original"
        );
    }

    /// Test case: Verifies behavior on a disconnected graph.
    ///
    /// Graph structure:
    ///   A, B (no edges)
    #[test]
    fn topological_sort_on_disconnected_graph() {
        let mut graph = ComputationGraph::new();

        graph.add_node(Node::Constant {
            value: vec![1.0],
            meta: NodeMetadata::default(),
        });
        graph.add_node(Node::Constant {
            value: vec![2.0],
            meta: NodeMetadata::default(),
        });

        let order = graph
            .topological_order()
            .expect("Sort should succeed on disconnected graph");

        assert_eq!(order.len(), 2, "Should include all nodes");
    }

    /// Test case: A more complex, multi-level DAG.
    ///
    /// Graph structure:
    ///   A ---v
    ///        D -> E
    ///   B -> C ---^
    #[test]
    fn topological_sort_on_multi_level_dag() {
        let mut graph = ComputationGraph::new();

        let node_a = graph.add_node(Node::Constant {
            value: vec![],
            meta: Default::default(),
        });
        let node_b = graph.add_node(Node::Constant {
            value: vec![],
            meta: Default::default(),
        });
        let node_c = graph.add_node(Node::Formula {
            op: Operation::Add,
            parents: vec![node_b],
            meta: Default::default(),
        });
        let node_d = graph.add_node(Node::Formula {
            op: Operation::Add,
            parents: vec![node_a],
            meta: Default::default(),
        });
        let node_e = graph.add_node(Node::Formula {
            op: Operation::Add,
            parents: vec![node_c, node_d],
            meta: Default::default(),
        });

        graph.add_dependency(node_a, node_d, Edge::Arithmetic);
        graph.add_dependency(node_b, node_c, Edge::Arithmetic);
        graph.add_dependency(node_c, node_e, Edge::Arithmetic);
        graph.add_dependency(node_d, node_e, Edge::Arithmetic);

        let order = graph.topological_order().expect("Sort should succeed");

        assert_eq!(order.len(), 5);

        let pos = |id| order.iter().position(|&n| n == id).unwrap();

        assert!(pos(node_c) > pos(node_b));
        assert!(pos(node_d) > pos(node_a));
        assert!(pos(node_e) > pos(node_c));
        assert!(pos(node_e) > pos(node_d));
    }
}