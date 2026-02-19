use crate::store::{Registry, NodeId};
use std::collections::HashSet;

/// Performs a Topological Sort using Depth-First Search (DFS).
///
/// Returns a list of NodeIds where every dependency appears before its consumer.
///
/// **Optimization:**
/// Uses DFS instead of BFS (Kahn's) to improve cache locality. In a deep dependency
/// chain A->B->C, DFS places [A, B, C] close together. BFS would place [Layer1, ..., Layer2],
/// causing A and B to be far apart in memory for wide graphs.
pub fn sort(registry: &Registry) -> Result<Vec<NodeId>, String> {
    let count = registry.count();
    let mut order = Vec::with_capacity(count);
    let mut state = vec![VisitState::None; count];

    // We iterate 0..count to ensure all nodes (even disconnected ones) are visited.
    // In a dependency graph, edges point Child -> Parent.
    // We want to process Parents before Children.
    // Standard DFS post-order on this structure gives [Parent, ..., Child].
    for i in 0..count {
        if state[i] == VisitState::None {
            visit(NodeId::new(i), registry, &mut state, &mut order)?;
        }
    }

    Ok(order)
}

#[derive(Clone, PartialEq, Eq)]
enum VisitState {
    None,
    Visiting, // Used for cycle detection
    Visited,
}

fn visit(
    node: NodeId,
    registry: &Registry,
    state: &mut Vec<VisitState>,
    order: &mut Vec<NodeId>,
) -> Result<(), String> {
    let idx = node.index();
    
    match state[idx] {
        VisitState::Visited => return Ok(()),
        VisitState::Visiting => return Err(format!("Cycle detected involving node {}", idx)),
        VisitState::None => state[idx] = VisitState::Visiting,
    }

    // Recurse on dependencies (Parents)
    // Note: registry.get_parents returns the upstream inputs for this node.
    for &parent in registry.get_parents(node) {
        visit(parent, registry, state, order)?;
    }

    state[idx] = VisitState::Visited;
    order.push(node);
    Ok(())
}

/// Identifies all nodes downstream from the given start nodes.
/// Used for incremental invalidation.
pub fn downstream_from(registry: &Registry, start_nodes: &[NodeId]) -> HashSet<NodeId> {
    use std::collections::VecDeque;
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


#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{NodeKind, NodeMetadata, Operation};

    fn make_meta(name: &str) -> NodeMetadata { 
        NodeMetadata { name: name.into(), ..Default::default() } 
    }

    #[test]
    fn test_sort_diamond_dependency() {
        // Shape: A -> B, A -> C, B+C -> D
        // Valid Orders: A,B,C,D or A,C,B,D
        let mut reg = Registry::new();
        let a = reg.add_node(NodeKind::Scalar(1.0), &[], make_meta("A"));
        let b = reg.add_node(NodeKind::Formula(Operation::Add), &[a, a], make_meta("B"));
        let c = reg.add_node(NodeKind::Formula(Operation::Add), &[a, a], make_meta("C"));
        let d = reg.add_node(NodeKind::Formula(Operation::Add), &[b, c], make_meta("D"));

        let res = sort(&reg).expect("Sort failed");
        
        let pos = |id: NodeId| res.iter().position(|&x| x == id).unwrap();
        assert!(pos(a) < pos(b));
        assert!(pos(a) < pos(c));
        assert!(pos(b) < pos(d));
        assert!(pos(c) < pos(d));
    }

    #[test]
    fn test_cycle_detection_explicit() {
        // Construct A -> B. Then force B -> A via internal mutation.
        let mut reg = Registry::new();
        let a = reg.add_node(NodeKind::Scalar(0.0), &[], make_meta("A")); // ID 0
        let b = reg.add_node(NodeKind::Formula(Operation::Add), &[a, a], make_meta("B")); // ID 1
        
        // HACK: Manually inject the cycle A depends on B.
        // A is at index 0. We verify it has no parents initially.
        assert_eq!(reg.parents_ranges[0].1, 0);
        
        // 1. Add 'B' (ID 1) to the flat parent list
        reg.parents_flat.push(b); 
        // 2. Point 'A' (Index 0) to this new parent entry
        let new_start = (reg.parents_flat.len() - 1) as u32;
        reg.parents_ranges[0] = (new_start, 1); 

        // Now A -> B and B -> A.
        let err = sort(&reg).unwrap_err();
        assert!(err.contains("Cycle detected"), "Msg: {}", err);
    }
}