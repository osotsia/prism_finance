//! dag.rs
//! Wraps the low-level GraphRegistry with high-level graph algorithms.
//! Optimized for Dense Columnar Layout (CSR Parents + LinkedList Children).

use super::storage::{GraphRegistry, NodeId, NodeKind};
use super::node::{NodeMetadata, Operation};
use std::collections::{HashSet, VecDeque};

#[derive(Debug, Clone, Default)]
pub struct ComputationGraph {
    pub(crate) store: GraphRegistry,
}

impl ComputationGraph {
    pub fn new() -> Self { Self::default() }

    pub fn add_constant(&mut self, value: Vec<f64>, meta: NodeMetadata) -> NodeId {
        self.store.add_constant(value, meta)
    }

    pub fn add_formula(&mut self, op: Operation, parents: Vec<NodeId>, meta: NodeMetadata) -> NodeId {
        self.store.add_formula(op, parents, meta)
    }

    pub fn add_solver_var(&mut self, meta: NodeMetadata) -> NodeId {
        self.store.add_solver_var(meta)
    }

    pub fn update_constant(&mut self, id: NodeId, val: Vec<f64>) -> Result<(), String> {
        self.store.update_constant(id, val)
    }

    pub fn get_constant_value(&self, id: NodeId) -> Option<&Vec<f64>> {
        self.store.get_constant(id)
    }

    pub fn node_count(&self) -> usize { self.store.count() }

    // --- Graph Algorithms ---

    /// Returns a topological sort using Kahn's Algorithm.
    pub fn topological_order(&self) -> Result<Vec<NodeId>, String> {
        let count = self.store.count();
        let mut in_degree = vec![0; count];
        let mut queue = VecDeque::with_capacity(count);
        let mut order = Vec::with_capacity(count);

        // 1. Initialize In-Degrees O(N)
        for (i, &(_, count)) in self.store.parents_ranges.iter().enumerate() {
            in_degree[i] = count as usize;
            if count == 0 {
                queue.push_back(NodeId::new(i));
            }
        }

        // 2. Process Queue
        while let Some(node) = queue.pop_front() {
            order.push(node);

            // Iterate Children (Linked List Traversal)
            let mut edge_idx = self.store.first_child[node.index()];
            while edge_idx != u32::MAX {
                let child = self.store.child_targets[edge_idx as usize];
                let child_idx = child.index();
                
                in_degree[child_idx] -= 1;
                if in_degree[child_idx] == 0 {
                    queue.push_back(child);
                }
                
                // Move to next sibling
                edge_idx = self.store.next_child[edge_idx as usize];
            }
        }

        if order.len() != count {
            return Err("Cycle detected in graph".to_string());
        }

        Ok(order)
    }

    pub fn downstream_from(&self, start_nodes: &[NodeId]) -> HashSet<NodeId> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::from(start_nodes.to_vec());

        while let Some(node) = queue.pop_front() {
            if visited.insert(node) {
                // Iterate Children (Linked List Traversal)
                let mut edge_idx = self.store.first_child[node.index()];
                while edge_idx != u32::MAX {
                    let child = self.store.child_targets[edge_idx as usize];
                    queue.push_back(child);
                    edge_idx = self.store.next_child[edge_idx as usize];
                }
            }
        }
        visited
    }

    pub fn upstream_from(&self, start_nodes: &[NodeId]) -> HashSet<NodeId> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::from(start_nodes.to_vec());

        while let Some(node) = queue.pop_front() {
            if visited.insert(node) {
                for &parent in self.store.get_parents(node) {
                    queue.push_back(parent);
                }
            }
        }
        visited
    }
    
    // --- Accessors ---
    pub fn get_node_kind(&self, id: NodeId) -> &NodeKind { &self.store.kinds[id.index()] }
    pub fn get_node_meta(&self, id: NodeId) -> &NodeMetadata { &self.store.meta[id.index()] }
    pub fn get_parents(&self, id: NodeId) -> &[NodeId] { self.store.get_parents(id) }
}