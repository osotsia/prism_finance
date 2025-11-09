//! A synchronous, single-threaded computation engine.
use crate::graph::{ComputationGraph, Node, NodeId, Operation};
use crate::computation::ledger::{ComputationError, Ledger};
use std::collections::HashSet;
use std::sync::Arc;
use std::cmp::max;

pub struct ComputationEngine<'a> {
    graph: &'a ComputationGraph,
}

impl<'a> ComputationEngine<'a> {
    pub fn new(graph: &'a ComputationGraph) -> Self {
        Self { graph }
    }

    /// Computes the values for a set of target nodes, calculating their dependencies as needed.
    /// This implementation uses a recursive DFS to build a valid evaluation order on the fly,
    /// respecting values that are already present in the ledger (e.g., constants, or solver variables).
    /// This allows it to work on the cyclic graph present during solver execution.
    pub fn compute(&self, targets: &[NodeId], ledger: &mut Ledger) -> Result<(), ComputationError> {
        let mut eval_order = Vec::new();
        let mut visiting = HashSet::new(); // For cycle detection
        let mut visited = HashSet::new();  // For memoization

        for &target_id in targets {
            self.build_eval_order_dfs(target_id, ledger, &mut eval_order, &mut visiting, &mut visited)?;
        }

        for &node_id in &eval_order {
            // The DFS ensures parents are evaluated first.
            let result = self.evaluate_node_with_parents(node_id, ledger);
            ledger.insert(node_id, result);
        }

        Ok(())
    }

    /// A recursive helper function to perform a depth-first search, building a post-order
    /// traversal of the dependency graph (which is a topological sort).
    fn build_eval_order_dfs(
        &self,
        node_id: NodeId,
        ledger: &Ledger,
        eval_order: &mut Vec<NodeId>,
        visiting: &mut HashSet<NodeId>,
        visited: &mut HashSet<NodeId>,
    ) -> Result<(), ComputationError> {
        // If the node has been fully processed or is already in the ledger, we're done.
        if visited.contains(&node_id) || ledger.get(node_id).is_some() {
            return Ok(());
        }
        // If we encounter a node currently in the recursion stack, we've found a cycle.
        if visiting.contains(&node_id) {
            return Err(ComputationError::CycleDetected);
        }

        visiting.insert(node_id);

        if let Some(Node::Formula { parents, .. }) = self.graph.get_node(node_id) {
            for &parent_id in parents {
                self.build_eval_order_dfs(parent_id, ledger, eval_order, visiting, visited)?;
            }
        }

        visiting.remove(&node_id);
        visited.insert(node_id);
        eval_order.push(node_id);
        Ok(())
    }

    fn evaluate_node_with_parents(&self, node_id: NodeId, ledger: &Ledger) -> Result<Arc<Vec<f64>>, ComputationError> {
        let node = self.graph.get_node(node_id).unwrap();
        match node {
            Node::Constant { .. } => {
                let val = self.graph.get_constant_value(node_id).cloned().unwrap_or_default();
                Ok(Arc::new(val))
            },
            Node::Formula { parents, .. } => {
                let mut parent_values = Vec::with_capacity(parents.len());
                for pid in parents.iter() {
                    match ledger.get(*pid).expect("BUG: Parent must be computed due to topological order") {
                        Ok(val) => parent_values.push(val.clone()),
                        Err(e) => return Err(ComputationError::UpstreamError {
                            node_id,
                            node_name: node.meta().name.clone(),
                            parent_id: *pid,
                            parent_name: self.graph.get_node(*pid).unwrap().meta().name.clone(),
                            source_error: Box::new(e.clone()),
                        }),
                    }
                }
                self.evaluate_formula(node_id, node, &parent_values)
            },
            Node::SolverVariable { .. } => Ok(Arc::new(vec![0.0])), // Default, will be overwritten
        }
    }

    fn evaluate_formula(&self, node_id: NodeId, node: &Node, parent_values: &[Arc<Vec<f64>>]) -> Result<Arc<Vec<f64>>, ComputationError> {
        let op = match node {
            Node::Formula { op, .. } => op,
            _ => unreachable!(),
        };

        let result_vec = match op {
            Operation::Add | Operation::Subtract | Operation::Multiply | Operation::Divide => {
                if parent_values.len() != 2 {
                    return Err(ComputationError::ParentCountMismatch { node_id, node_name: node.meta().name.clone(), op: format!("{:?}", op), expected: 2, actual: parent_values.len() });
                }
                let lhs = &parent_values[0];
                let rhs = &parent_values[1];
                let len = max(lhs.len(), rhs.len());
                let mut result = Vec::with_capacity(len);
                for i in 0..len {
                    let l = *lhs.get(i).unwrap_or_else(|| lhs.last().unwrap_or(&0.0));
                    let r = *rhs.get(i).unwrap_or_else(|| rhs.last().unwrap_or(&0.0));
                    match op {
                        Operation::Add => result.push(l + r),
                        Operation::Subtract => result.push(l - r),
                        Operation::Multiply => result.push(l * r),
                        Operation::Divide => {
                            if r == 0.0 { return Err(ComputationError::DivisionByZero { node_id, node_name: node.meta().name.clone() }); }
                            result.push(l / r);
                        }
                        _ => unreachable!(),
                    }
                }
                result
            }
            Operation::PreviousValue { lag, .. } => {
                if parent_values.len() != 2 {
                    return Err(ComputationError::ParentCountMismatch { node_id, node_name: node.meta().name.clone(), op: "PreviousValue".to_string(), expected: 2, actual: parent_values.len() });
                }
                let main_series = &parent_values[0];
                let default_series = &parent_values[1];
                let lag_usize = *lag as usize;
                let len = main_series.len();
                let mut result = Vec::with_capacity(len);
                for i in 0..len {
                    if i < lag_usize {
                        result.push(*default_series.get(i).unwrap_or_else(|| default_series.last().unwrap_or(&0.0)));
                    } else {
                        result.push(main_series[i - lag_usize]);
                    }
                }
                result
            }
        };

        Ok(Arc::new(result_vec))
    }
}