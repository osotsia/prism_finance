//! The core calculation engine.
//!
//! This module defines `ComputationEngine`, a synchronous, single-threaded executor for the
//! calculation graph. Its primary design feature is its ability to compute values on-demand
//! based on the current state of a `Ledger`.
//!
//! Unlike a simple Directed Acyclic Graph (DAG) traversal, this engine builds its evaluation
//! order dynamically. This allows it to function correctly even on graphs that are temporarily
//! cyclic during solver execution, by respecting the pre-calculated or guessed values already
//! present in the ledger. It effectively calculates only what is necessary to reach the target nodes.

use crate::computation::ledger::{ComputationError, Ledger};
use crate::graph::{ComputationGraph, Node, NodeId, Operation};
use std::cmp::max;
use std::collections::HashSet;
use std::sync::Arc;

/// A synchronous, single-threaded computation engine.
pub struct ComputationEngine<'a> {
    graph: &'a ComputationGraph,
}

impl<'a> ComputationEngine<'a> {
    pub fn new(graph: &'a ComputationGraph) -> Self {
        Self { graph }
    }

    /// The "Orchestrator": Computes values for target nodes and stores them in the ledger.
    ///
    /// This method orchestrates the two main phases of computation:
    /// 1.  **Planning:** It determines a valid, minimal sequence of calculations required
    ///     to compute the targets, respecting any values already in the ledger.
    /// 2.  **Execution:** It carries out the plan, evaluating each node in order.
    pub fn compute(&self, targets: &[NodeId], ledger: &mut Ledger) -> Result<(), ComputationError> {
        // Phase 1: Determine the sequence of operations.
        let execution_plan = self.plan_execution_order(targets, ledger)?;

        // Phase 2: Execute the plan and populate the ledger.
        self.execute_plan(&execution_plan, ledger)
    }

    // --- Private: Planning Phase ---

    /// The "Planner": Generates a valid evaluation order (a topological sort) for the
    /// required computations using a recursive Depth-First Search (DFS).
    fn plan_execution_order(
        &self,
        targets: &[NodeId],
        ledger: &Ledger,
    ) -> Result<Vec<NodeId>, ComputationError> {
        let mut plan = Vec::new();
        let mut visiting = HashSet::new(); // For cycle detection.
        let mut visited = HashSet::new(); // For memoization to avoid re-processing nodes.

        for &target_id in targets {
            self.build_plan_recursive(target_id, ledger, &mut plan, &mut visiting, &mut visited)?;
        }
        Ok(plan)
    }

    /// Recursively builds the execution plan (a post-order traversal of the dependency graph).
    fn build_plan_recursive(
        &self,
        node_id: NodeId,
        ledger: &Ledger,
        plan: &mut Vec<NodeId>,
        visiting: &mut HashSet<NodeId>,
        visited: &mut HashSet<NodeId>,
    ) -> Result<(), ComputationError> {
        // If the node's value is already known (in the ledger) or its dependencies
        // have been fully processed, we don't need to do anything further.
        if visited.contains(&node_id) || ledger.get(node_id).is_some() {
            return Ok(());
        }
        // If we encounter a node that is currently in the recursion stack, we've found a cycle
        // that isn't broken by a pre-existing ledger value. This is a fatal structural error.
        if visiting.contains(&node_id) {
            return Err(ComputationError::CycleDetected);
        }

        visiting.insert(node_id);

        // Recursively build the plan for all parent dependencies.
        if let Some(Node::Formula { parents, .. }) = self.graph.get_node(node_id) {
            for &parent_id in parents {
                self.build_plan_recursive(parent_id, ledger, plan, visiting, visited)?;
            }
        }

        visiting.remove(&node_id);
        visited.insert(node_id);
        // Add the node to the plan only after all its dependencies have been added.
        plan.push(node_id);
        Ok(())
    }

    // --- Private: Execution Phase ---

    /// The "Executor": Iterates through the planned nodes and computes their values.
    fn execute_plan(&self, plan: &[NodeId], ledger: &mut Ledger) -> Result<(), ComputationError> {
        for &node_id in plan {
            // The plan guarantees that when we evaluate a node, its parents are already in the ledger.
            let result = self.compute_single_node(node_id, ledger);
            ledger.insert(node_id, result);
        }
        Ok(())
    }

    /// Computes the value for a single node, assuming its parents are in the ledger.
    fn compute_single_node(
        &self,
        node_id: NodeId,
        ledger: &Ledger,
    ) -> Result<Arc<Vec<f64>>, ComputationError> {
        // The node must exist in the graph if it's in our execution plan.
        let node = self.graph.get_node(node_id).unwrap();

        match node {
            Node::Constant { .. } => {
                // For constants, the value is stored directly in the graph structure.
                let val = self.graph.get_constant_value(node_id).cloned().unwrap_or_default();
                Ok(Arc::new(val))
            }
            Node::Formula { parents, op, .. } => {
                // For formulas, we first gather the computed values of its parents from the ledger.
                // Using Arc avoids deep-copying potentially large time-series vectors.
                let parent_values = parents
                    .iter()
                    .map(|&pid| {
                        match ledger.get(pid) {
                            // This expect is safe due to the topological ordering of the execution plan.
                            Some(Ok(val)) => Ok(val.clone()),
                            Some(Err(e)) => Err(ComputationError::UpstreamError {
                                node_id,
                                node_name: node.meta().name.clone(),
                                parent_id: pid,
                                parent_name: self.graph.get_node(pid).unwrap().meta().name.clone(),
                                source_error: Box::new(e.clone()),
                            }),
                            None => panic!("BUG: Parent must be computed before child."),
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                self.evaluate_formula(node_id, op, &parent_values)
            }
            Node::SolverVariable { .. } => {
                // Solver variables are placeholders. If the engine is asked to compute one, it means
                // it wasn't pre-filled by the solver, so we provide a default value.
                Ok(Arc::new(vec![0.0]))
            }
        }
    }

    /// Performs the specific mathematical operation for a `Formula` node.
    fn evaluate_formula(
        &self,
        node_id: NodeId,
        op: &Operation,
        parent_values: &[Arc<Vec<f64>>],
    ) -> Result<Arc<Vec<f64>>, ComputationError> {
        let node_name = self.graph.get_node(node_id).unwrap().meta().name.clone();

        let result_vec = match op {
            Operation::Add | Operation::Subtract | Operation::Multiply | Operation::Divide => {
                if parent_values.len() != 2 {
                    return Err(ComputationError::ParentCountMismatch { node_id, node_name, op: format!("{:?}", op), expected: 2, actual: parent_values.len() });
                }
                let lhs = &parent_values[0];
                let rhs = &parent_values[1];
                let len = max(lhs.len(), rhs.len());
                let mut result = Vec::with_capacity(len);

                for i in 0..len {
                    // This handles time-series broadcasting: if one input is a scalar (len=1) and
                    // the other is a vector, the scalar's last value is used for all time steps.
                    let l = get_broadcast_value(lhs, i);
                    let r = get_broadcast_value(rhs, i);
                    match op {
                        Operation::Add => result.push(l + r),
                        Operation::Subtract => result.push(l - r),
                        Operation::Multiply => result.push(l * r),
                        Operation::Divide => {
                            if r == 0.0 { return Err(ComputationError::DivisionByZero { node_id, node_name }); }
                            result.push(l / r);
                        }
                        _ => unreachable!(),
                    }
                }
                result
            }
            Operation::PreviousValue { lag, .. } => {
                if parent_values.len() != 2 {
                    return Err(ComputationError::ParentCountMismatch { node_id, node_name, op: "PreviousValue".to_string(), expected: 2, actual: parent_values.len() });
                }
                let main_series = &parent_values[0];
                let default_series = &parent_values[1];
                let lag_usize = *lag as usize;
                let len = main_series.len();
                let mut result = Vec::with_capacity(len);

                for i in 0..len {
                    if i < lag_usize {
                        result.push(get_broadcast_value(default_series, i));
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

/// A helper to retrieve a value from a vector for a given time-step `i`,
/// broadcasting the last available value if `i` is out of bounds.
#[inline]
fn get_broadcast_value(series: &[f64], i: usize) -> f64 {
    *series.get(i).unwrap_or_else(|| series.last().unwrap_or(&0.0))
}