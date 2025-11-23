//! engine.rs
//! 
//! Architecture:
//! 1. Planner: Topo-sorts the graph (DFS).
//! 2. Executor: Iterates the plan, fetches inputs.
//! 3. Kernel: Pure function `compute_kernel` performs math on `Value` enum.
//!    - Parallel Ready: The Kernel has no side effects and no access to the graph/ledger.

use crate::computation::ledger::{ComputationError, Ledger, Value};
use crate::graph::{ComputationGraph, NodeId, NodeKind, Operation};
use std::cmp::max;
use std::sync::Arc;

pub struct ComputationEngine<'a> {
    graph: &'a ComputationGraph,
}

impl<'a> ComputationEngine<'a> {
    pub fn new(graph: &'a ComputationGraph) -> Self { Self { graph } }

    pub fn compute(&self, targets: &[NodeId], ledger: &mut Ledger) -> Result<(), ComputationError> {
        ledger.ensure_capacity(self.graph.node_count());
        let plan = self.plan_execution_order(targets, ledger)?;
        self.execute_plan(&plan, ledger)
    }

    // --- Phase 1: Planner (DFS) ---
    fn plan_execution_order(
        &self,
        targets: &[NodeId],
        ledger: &Ledger,
    ) -> Result<Vec<NodeId>, ComputationError> {
        let mut plan = Vec::new();
        // 0=Unvisited, 1=Visiting, 2=Visited
        let mut state = vec![0u8; self.graph.node_count()];

        for &target_id in targets {
            self.build_plan_recursive(target_id, ledger, &mut plan, &mut state)?;
        }
        Ok(plan)
    }

    fn build_plan_recursive(
        &self,
        node_id: NodeId,
        ledger: &Ledger,
        plan: &mut Vec<NodeId>,
        state: &mut Vec<u8>,
    ) -> Result<(), ComputationError> {
        let idx = node_id.index();

        if state[idx] == 2 || ledger.get(node_id).is_some() {
            return Ok(());
        }
        if state[idx] == 1 {
            return Err(ComputationError::CycleDetected);
        }

        state[idx] = 1;
        for &parent_id in self.graph.store.get_parents(node_id) {
            self.build_plan_recursive(parent_id, ledger, plan, state)?;
        }
        state[idx] = 2;
        plan.push(node_id);
        Ok(())
    }

    // --- Phase 2: Executor ---
    fn execute_plan(&self, plan: &[NodeId], ledger: &mut Ledger) -> Result<(), ComputationError> {
        // FUTURE PARALLELISM: 
        // 1. Group `plan` into independent waves/levels.
        // 2. plan.par_iter().for_each(...)
        for &node_id in plan {
            let result = self.compute_single_node(node_id, ledger);
            ledger.insert(node_id, result);
        }
        Ok(())
    }

    fn compute_single_node(
        &self,
        node_id: NodeId,
        ledger: &Ledger,
    ) -> Result<Value, ComputationError> {
        let kind = self.graph.get_node_kind(node_id);

        match kind {
            // Optimization: Zero-copy scalar access
            NodeKind::Scalar(val) => Ok(Value::Scalar(*val)),
            
            NodeKind::TimeSeries(idx) => {
                let vec_ref = &self.graph.store.constants_data[*idx as usize];
                // Clone the Vec into Arc (Cheap if we assume mostly scalars in model)
                // If heavy time-series usage, we might optimize storage to return Arc directly.
                Ok(Value::Series(Arc::new(vec_ref.clone())))
            }

            NodeKind::Formula(op) => {
                let parents = self.graph.store.get_parents(node_id);
                
                // Collect inputs (Read-Only access to Ledger)
                // Small stack allocation for parent values (usually 2 pointers)
                let mut parent_values = smallvec::SmallVec::<[&Value; 2]>::new();
                
                for &pid in parents {
                    match ledger.get(pid) {
                        Some(Ok(val)) => parent_values.push(val),
                        Some(Err(e)) => return Err(ComputationError::UpstreamError {
                            node_id,
                            node_name: self.graph.get_node_meta(node_id).name.clone(),
                            parent_id: pid,
                            parent_name: self.graph.get_node_meta(pid).name.clone(),
                            source_error: Box::new(e.clone()),
                        }),
                        None => panic!("Bug: Scheduler missed dependency"),
                    }
                }
                
                // DELEGATE TO PURE KERNEL
                let node_name = &self.graph.get_node_meta(node_id).name;
                compute_kernel(node_id, node_name, op, &parent_values)
            }

            NodeKind::SolverVariable => Ok(Value::Scalar(0.0)),
        }
    }
}

// --- The Compute Kernel (Pure Function) ---
// This function has NO dependency on the Graph or Ledger struct.
// It is perfectly thread-safe and cache-friendly.
// --- The Compute Kernel (Pure Function) ---
fn compute_kernel(
    node_id: NodeId,
    node_name: &str,
    op: &Operation,
    inputs: &[&Value],
) -> Result<Value, ComputationError> {
    
    match op {
        Operation::Add | Operation::Subtract | Operation::Multiply | Operation::Divide => {
            if inputs.len() != 2 {
                return Err(ComputationError::ParentCountMismatch { 
                    node_id, node_name: node_name.into(), expected: 2, actual: inputs.len() 
                });
            }
            let (lhs, rhs) = (inputs[0], inputs[1]);

            // Branch: Scalar vs Scalar (Fastest path)
            if let (Value::Scalar(l), Value::Scalar(r)) = (lhs, rhs) {
                 return match op {
                    // FIXED: Was l + l
                    Operation::Add => Ok(Value::Scalar(l + r)), 
                    Operation::Subtract => Ok(Value::Scalar(l - r)),
                    Operation::Multiply => Ok(Value::Scalar(l * r)),
                    Operation::Divide => {
                        if *r == 0.0 { Err(ComputationError::DivisionByZero{ node_id, node_name: node_name.into() }) } 
                        else { Ok(Value::Scalar(l / r)) }
                    },
                    _ => unreachable!(),
                 };
            }

            // Branch: Broadcasting (Slow path)
            let (l_len, l_is_scalar) = match lhs { Value::Scalar(_) => (1, true), Value::Series(v) => (v.len(), false) };
            let (r_len, r_is_scalar) = match rhs { Value::Scalar(_) => (1, true), Value::Series(v) => (v.len(), false) };
            let len = max(l_len, r_len);
            
            let mut result = Vec::with_capacity(len);
            
            let l_val = if l_is_scalar { if let Value::Scalar(v) = lhs { *v } else { 0.0 } } else { 0.0 };
            let r_val = if r_is_scalar { if let Value::Scalar(v) = rhs { *v } else { 0.0 } } else { 0.0 };
            
            for i in 0..len {
                let l = if l_is_scalar { l_val } else { get_vec_val(lhs, i) };
                let r = if r_is_scalar { r_val } else { get_vec_val(rhs, i) };
                
                match op {
                    Operation::Add => result.push(l + r),
                    Operation::Subtract => result.push(l - r),
                    Operation::Multiply => result.push(l * r),
                    Operation::Divide => {
                        if r == 0.0 { return Err(ComputationError::DivisionByZero{ node_id, node_name: node_name.into() }) } 
                        else { result.push(l / r); }
                    },
                    _ => unreachable!(),
                }
            }
            Ok(Value::Series(Arc::new(result)))
        }

        Operation::PreviousValue { lag, .. } => {
            if inputs.len() != 2 { return Err(ComputationError::ParentCountMismatch { node_id, node_name: node_name.into(), expected: 2, actual: inputs.len() }); }
            let (main, def) = (inputs[0], inputs[1]);
            
            let main_len = match main { Value::Scalar(_) => 1, Value::Series(v) => v.len() };
            let def_len = match def { Value::Scalar(_) => 1, Value::Series(v) => v.len() };
            let len = max(main_len, def_len); 
            
            let mut result = Vec::with_capacity(len);
            let lag_u = *lag as usize;
            
            for i in 0..len {
                if i < lag_u {
                    result.push(get_val_at(def, i));
                } else {
                    result.push(get_val_at(main, i - lag_u));
                }
            }
            Ok(Value::Series(Arc::new(result)))
        }
    }
}

#[inline(always)]
fn get_vec_val(v: &Value, i: usize) -> f64 {
    match v {
        Value::Scalar(s) => *s, // Should be handled by hoist, but safety fallback
        Value::Series(vec) => *vec.get(i).unwrap_or_else(|| vec.last().unwrap_or(&0.0))
    }
}

#[inline(always)]
fn get_val_at(v: &Value, i: usize) -> f64 {
    match v {
        Value::Scalar(s) => *s,
        Value::Series(vec) => *vec.get(i).unwrap_or_else(|| vec.last().unwrap_or(&0.0))
    }
}