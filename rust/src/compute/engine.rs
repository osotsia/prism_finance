use crate::store::{Registry, NodeId, NodeKind};
use super::ledger::{Ledger, ComputationError, Value};
use super::kernel;
use std::sync::Arc;
use smallvec::SmallVec;

pub struct Engine<'a> {
    registry: &'a Registry,
}

impl<'a> Engine<'a> {
    pub fn new(registry: &'a Registry) -> Self { Self { registry } }

    pub fn compute(&self, targets: &[NodeId], ledger: &mut Ledger) -> Result<(), ComputationError> {
        ledger.ensure_capacity(self.registry.count());
        let plan = self.plan(targets, ledger)?;
        
        for node_id in plan {
            let res = self.compute_node(node_id, ledger);
            ledger.insert(node_id, res);
        }
        Ok(())
    }

    fn plan(&self, targets: &[NodeId], ledger: &Ledger) -> Result<Vec<NodeId>, ComputationError> {
        let mut plan = Vec::new();
        // 0=Unvisited, 1=Visiting, 2=Visited
        let mut state = vec![0u8; self.registry.count()];

        for &t in targets {
            self.dfs(t, ledger, &mut plan, &mut state)?;
        }
        Ok(plan)
    }

    fn dfs(&self, node: NodeId, ledger: &Ledger, plan: &mut Vec<NodeId>, state: &mut Vec<u8>) -> Result<(), ComputationError> {
        let idx = node.index();
        if state[idx] == 2 || ledger.get(node).is_some() { return Ok(()); }
        if state[idx] == 1 { return Err(ComputationError::CycleDetected); }

        state[idx] = 1;
        for &parent in self.registry.get_parents(node) {
            self.dfs(parent, ledger, plan, state)?;
        }
        state[idx] = 2;
        plan.push(node);
        Ok(())
    }

    fn compute_node(&self, node: NodeId, ledger: &Ledger) -> Result<Value, ComputationError> {
        let kind = &self.registry.kinds[node.index()];

        match kind {
            NodeKind::Scalar(v) => Ok(Value::Scalar(*v)),
            NodeKind::TimeSeries(idx) => {
                let vec_ref = &self.registry.constants_data[*idx as usize];
                Ok(Value::Series(Arc::new(vec_ref.clone())))
            }
            NodeKind::Formula(op) => {
                let parents = self.registry.get_parents(node);
                let mut inputs = SmallVec::<[&Value; 2]>::new();

                for &p in parents {
                    match ledger.get(p) {
                        Some(Ok(v)) => inputs.push(v),
                        Some(Err(e)) => return Err(ComputationError::Upstream(format!("{:?}", e))),
                        None => panic!("Scheduler error: dependency missing"),
                    }
                }

                let debug_name = &self.registry.meta[node.index()].name;
                kernel::execute(op, &inputs, debug_name)
            }
            NodeKind::SolverVariable => Ok(Value::Scalar(0.0)),
        }
    }
}