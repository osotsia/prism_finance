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
    use crate::graph::node::NodeMetadata;
    use rstest::rstest;

    // --- 1. Refined Test Data Structures ---
    // Using a tuple variant for `Success` makes test case definitions much more concise.
    #[derive(Debug)]
    enum Expectation {
        /// Sort should succeed. Contains `(expected_node_count, order_assertions)`.
        Success(usize, Vec<(usize, usize)>),
        /// Sort should fail due to a cycle.
        Cycle,
    }

    // --- 2. The Parameterized Test Function ---
    #[rstest]
    #[case("Valid simple DAG",     3, vec![(0, 2), (1, 2)],         Expectation::Success(3, vec![(0, 2), (1, 2)]))] // A->C, B->C
    #[case("Cyclic graph",         3, vec![(0, 1), (1, 2), (2, 0)], Expectation::Cycle)] // A->B->C->A
    #[case("Disconnected graph",   2, vec![],                       Expectation::Success(2, vec![]))]
    #[case("Multi-level DAG",      5, vec![(0, 3), (1, 2), (2, 4), (3, 4)], Expectation::Success(5, vec![(0, 3), (1, 2), (2, 4), (3, 4)]))] // A->D->E, B->C->E
    #[case("Linear chain",         4, vec![(0, 1), (1, 2), (2, 3)], Expectation::Success(4, vec![(0, 1), (1, 2), (2, 3)]))] // A->B->C->D
    fn test_topological_sort_scenarios(
        #[case] name: &str,
        #[case] num_nodes: usize,
        #[case] edges: Vec<(usize, usize)>,
        #[case] expectation: Expectation,
    ) {
        // --- Setup ---
        let mut graph = ComputationGraph::new();
        let node_ids: Vec<NodeId> = (0..num_nodes)
            .map(|_| graph.add_node(Node::Constant { value: vec![], meta: Default::default() }))
            .collect();
        for &(from_idx, to_idx) in &edges {
            graph.add_dependency(node_ids[from_idx], node_ids[to_idx], Edge::Arithmetic);
        }

        // --- Execution ---
        let result = graph.topological_order();

        // --- Assertion ---
        match expectation {
            Expectation::Success(expected_node_count, order_assertions) => {
                // If success is expected, the result must be Ok.
                let order = result.unwrap_or_else(|e| {
                    panic!("[{}] Expected success, but sort failed with: {:?}", name, e)
                });

                assert_eq!(order.len(), expected_node_count, "[{}] Incorrect node count.", name);

                for &(before_idx, after_idx) in &order_assertions {
                    let pos_before = order.iter().position(|&id| id == node_ids[before_idx]).unwrap();
                    let pos_after = order.iter().position(|&id| id == node_ids[after_idx]).unwrap();
                    assert!(
                        pos_before < pos_after,
                        "[{}] Order violation: {} should be before {}.", name, before_idx, after_idx
                    );
                }
            }
            Expectation::Cycle => {
                // If a cycle is expected, the result must be Err.
                assert!(result.is_err(), "[{}] Expected a cycle, but sort succeeded.", name);
            }
        }
    }

    /// This test is kept separate as it validates data integrity, not graph topology.
    /// Verifies adding and retrieving nodes by ID.
    #[test]
    fn add_and_get_node() {
        let mut graph = ComputationGraph::new();
        let meta = NodeMetadata { name: "Test Node".to_string(), ..Default::default() };
        let node_data = Node::Constant { value: vec![1.0], meta: meta.clone() };

        let node_id = graph.add_node(node_data.clone());
        let retrieved_node = graph.get_node(node_id).expect("Node should be retrievable");

        assert_eq!(*retrieved_node, node_data, "Retrieved node data does not match original");
    }
}