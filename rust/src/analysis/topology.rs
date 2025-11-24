use crate::store::{Registry, NodeId};
use std::collections::{VecDeque, HashSet};

/// Returns a topological sort using Kahn's Algorithm.
pub fn sort(registry: &Registry) -> Result<Vec<NodeId>, String> {
    let count = registry.count();
    let mut in_degree = vec![0; count];
    let mut queue = VecDeque::with_capacity(count);
    let mut order = Vec::with_capacity(count);

    // 1. Calculate In-Degrees
    for (i, &(_, count)) in registry.parents_ranges.iter().enumerate() {
        in_degree[i] = count as usize;
        if count == 0 {
            queue.push_back(NodeId::new(i));
        }
    }

    // 2. Process Queue
    while let Some(node) = queue.pop_front() {
        order.push(node);

        // Walk linked-list of children
        let mut edge_idx = registry.first_child[node.index()];
        while edge_idx != u32::MAX {
            let child = registry.child_targets[edge_idx as usize];
            let child_idx = child.index();
            
            in_degree[child_idx] -= 1;
            if in_degree[child_idx] == 0 {
                queue.push_back(child);
            }
            edge_idx = registry.next_child[edge_idx as usize];
        }
    }

    if order.len() != count {
        return Err("Cycle detected in graph".to_string());
    }

    Ok(order)
}

pub fn downstream_from(registry: &Registry, start_nodes: &[NodeId]) -> HashSet<NodeId> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::from(start_nodes.to_vec());

    while let Some(node) = queue.pop_front() {
        if visited.insert(node) {
            let mut edge_idx = registry.first_child[node.index()];
            while edge_idx != u32::MAX {
                let child = registry.child_targets[edge_idx as usize];
                queue.push_back(child);
                edge_idx = registry.next_child[edge_idx as usize];
            }
        }
    }
    visited
}