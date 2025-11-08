//! Implements the `ComputationGraph`, the primary data structure for storing the model's logic.

use super::edge::Edge;
use super::node::{Node, NodeId, NodeMetadata, Operation};
use petgraph::{algo::toposort, stable_graph::StableDiGraph, visit::Bfs};
use std::collections::{HashMap, HashSet};
/// A computation graph representing the financial model.
///
/// This structure acts as a "blueprint" of the model. It contains the
/// graph topology (`graph`) and the input data for constant nodes (`constants`).
/// This separation of structure from data is a "columnar" design, which
/// improves performance for the computation engine.
#[derive(Debug, Clone, Default)]
pub struct ComputationGraph {
    pub(crate) graph: StableDiGraph<Node, Edge>,
    pub(crate) constants: HashMap<NodeId, Vec<f64>>,
}

impl ComputationGraph {
    /// Creates a new, empty computation graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a `Constant` node and its associated data to the graph.
    pub fn add_constant(&mut self, value: Vec<f64>, meta: NodeMetadata) -> NodeId {
        let node = Node::Constant { meta };
        let node_id = self.graph.add_node(node);
        self.constants.insert(node_id, value);
        node_id
    }

    /// Adds a `Formula` node to the graph and establishes its dependencies.
    pub fn add_formula(
        &mut self,
        op: Operation,
        parents: Vec<NodeId>,
        meta: NodeMetadata,
    ) -> NodeId {
        let node = Node::Formula { op, parents: parents.clone(), meta };
        let child_id = self.graph.add_node(node);

        for parent_id in parents {
            self.add_dependency(parent_id, child_id, Edge::Arithmetic);
        }

        child_id
    }

    /// Adds a directed dependency (an edge) from a parent node to a child node.
    pub fn add_dependency(&mut self, parent: NodeId, child: NodeId, edge_type: Edge) {
        self.graph.add_edge(parent, child, edge_type);
    }

    /// Retrieves a reference to a node's data by its ID.
    pub fn get_node(&self, id: NodeId) -> Option<&Node> {
        self.graph.node_weight(id)
    }

    /// Retrieves a constant's time-series value by its ID.
    pub fn get_constant_value(&self, id: NodeId) -> Option<&Vec<f64>> {
        self.constants.get(&id)
    }

    /// Returns a vector of node IDs in a valid evaluation order (topological sort).
    pub fn topological_order(&self) -> Result<Vec<NodeId>, petgraph::algo::Cycle<NodeId>> {
        toposort(&self.graph, None)
    }

    /// Returns the total number of nodes currently in the graph.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Returns a set of all node IDs that are downstream of (i.e., depend on)
    /// any of the nodes in the initial set. Includes the initial nodes.
    pub fn downstream_from(&self, initial_set: &[NodeId]) -> HashSet<NodeId> {
        let mut downstream = HashSet::new();
        for &start_node in initial_set {
            if downstream.contains(&start_node) {
                continue;
            }
            let mut bfs = Bfs::new(&self.graph, start_node);
            while let Some(node_id) = bfs.next(&self.graph) {
                downstream.insert(node_id);
            }
        }
        downstream
    }
}

// --- Improved Test Suite ---
#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // --- 1. Test Data Structures & Helpers ---

    /// A helper to create a graph with N constant nodes for test setup.
    fn create_graph_with_constants(n: usize) -> (ComputationGraph, Vec<NodeId>) {
        let mut graph = ComputationGraph::new();
        let node_ids = (0..n)
            .map(|i| {
                graph.add_constant(
                    vec![],
                    NodeMetadata { name: format!("Node_{}", i), ..Default::default() },
                )
            })
            .collect();
        (graph, node_ids)
    }

    // --- 2. Topological Sort Tests ---

    #[derive(Debug)]
    enum TopoSortExpectation {
        /// Sort should succeed. Contains the expected final node count.
        Success(usize),
        /// Sort should fail due to a cycle.
        Cycle,
    }

    #[rstest]
    // Happy Path Scenarios
    #[case("Linear chain (A->B->C)", 3, vec![(0, 1), (1, 2)], TopoSortExpectation::Success(3))]
    #[case("Simple fork (A->B, A->C)", 3, vec![(0, 1), (0, 2)], TopoSortExpectation::Success(3))]
    #[case("Simple join (A->C, B->C)", 3, vec![(0, 2), (1, 2)], TopoSortExpectation::Success(3))]
    #[case("Diamond shape (A->B, A->C, B->D, C->D)", 4, vec![(0, 1), (0, 2), (1, 3), (2, 3)], TopoSortExpectation::Success(4))]
    #[case("Disconnected graph", 2, vec![], TopoSortExpectation::Success(2))]
    // Failure Scenario
    #[case("Cyclic graph (A->B->C->A)", 3, vec![(0, 1), (1, 2), (2, 0)], TopoSortExpectation::Cycle)]
    fn test_topological_sort(
        #[case] name: &str,
        #[case] num_nodes: usize,
        #[case] edges: Vec<(usize, usize)>,
        #[case] expectation: TopoSortExpectation,
    ) {
        // --- Setup ---
        let (mut graph, node_ids) = create_graph_with_constants(num_nodes);
        for &(from_idx, to_idx) in &edges {
            graph.add_dependency(node_ids[from_idx], node_ids[to_idx], Edge::Arithmetic);
        }

        // --- Execution ---
        let result = graph.topological_order();

        // --- Assertion ---
        match expectation {
            TopoSortExpectation::Success(expected_count) => {
                let order = result.unwrap_or_else(|e| panic!("[{}] Expected success, but sort failed with: {:?}", name, e));
                assert_eq!(order.len(), expected_count, "[{}] Incorrect node count.", name);
                // Verify ordering for all specified edges
                for &(before_idx, after_idx) in &edges {
                    let pos_before = order.iter().position(|&id| id == node_ids[before_idx]).unwrap();
                    let pos_after = order.iter().position(|&id| id == node_ids[after_idx]).unwrap();
                    assert!(pos_before < pos_after, "[{}] Order violation: {} should be before {}.", name, before_idx, after_idx);
                }
            }
            TopoSortExpectation::Cycle => {
                assert!(result.is_err(), "[{}] Expected a cycle, but sort succeeded.", name);
            }
        }
    }

    // --- 3. Node & Data Integrity Tests ---

    #[test]
    fn test_add_constant_stores_node_and_data_correctly() {
        let mut graph = ComputationGraph::new();
        let meta = NodeMetadata { name: "Test Node".to_string(), ..Default::default() };
        let node_data = Node::Constant { meta: meta.clone() };
        let value_data = vec![1.0, 2.0];

        let node_id = graph.add_constant(value_data.clone(), meta);
        
        // Assert node exists in the graph topology
        let retrieved_node = graph.get_node(node_id).expect("Node should be retrievable");
        assert_eq!(*retrieved_node, node_data, "Retrieved node data does not match original");

        // Assert its value exists in the columnar data store
        let retrieved_value = graph.get_constant_value(node_id).expect("Value should be retrievable");
        assert_eq!(*retrieved_value, value_data, "Retrieved value data does not match original");
    }

    // --- 4. Formula and Dependency Tests ---

    #[rstest]
    #[case(Operation::Add)]
    #[case(Operation::Subtract)]
    #[case(Operation::Multiply)]
    #[case(Operation::Divide)]
    fn test_add_formula_creates_correct_arithmetic_dependencies(#[case] op: Operation) {
        let (mut graph, node_ids) = create_graph_with_constants(2);
        let parent_a = node_ids[0];
        let parent_b = node_ids[1];

        let formula_id = graph.add_formula(
            op.clone(),
            vec![parent_a, parent_b],
            NodeMetadata { name: "Formula".to_string(), ..Default::default() },
        );

        // Assert node is correct
        let formula_node = graph.get_node(formula_id).unwrap();
        if let Node::Formula { op: node_op, parents, .. } = formula_node {
            assert_eq!(*node_op, op);
            assert_eq!(*parents, vec![parent_a, parent_b]);
        } else {
            panic!("Expected a Formula node");
        }

        // Assert dependencies are correct
        let edge1 = graph.graph.find_edge(parent_a, formula_id).expect("Edge from parent A should exist");
        assert_eq!(*graph.graph.edge_weight(edge1).unwrap(), Edge::Arithmetic);
        
        let edge2 = graph.graph.find_edge(parent_b, formula_id).expect("Edge from parent B should exist");
        assert_eq!(*graph.graph.edge_weight(edge2).unwrap(), Edge::Arithmetic);
    }

    #[test]
    fn test_can_construct_previous_value_structure_with_correct_edges() {
        let (mut graph, node_ids) = create_graph_with_constants(2);
        let main_node_id = node_ids[0];
        let default_node_id = node_ids[1];

        // Manually construct the structure, simulating the FFI layer's actions
        let formula_node = Node::Formula {
            op: Operation::PreviousValue { lag: 1, default_node: default_node_id },
            parents: vec![main_node_id, default_node_id],
            meta: NodeMetadata { name: "Prev".to_string(), ..Default::default() },
        };
        let formula_id = graph.graph.add_node(formula_node);
        graph.add_dependency(main_node_id, formula_id, Edge::Temporal);
        graph.add_dependency(default_node_id, formula_id, Edge::DefaultValue);
        
        // Assert the edges have the correct semantic types
        let temporal_edge_idx = graph.graph.find_edge(main_node_id, formula_id).unwrap();
        assert_eq!(*graph.graph.edge_weight(temporal_edge_idx).unwrap(), Edge::Temporal);

        let default_edge_idx = graph.graph.find_edge(default_node_id, formula_id).unwrap();
        assert_eq!(*graph.graph.edge_weight(default_edge_idx).unwrap(), Edge::DefaultValue);
    }
}