use crate::store::{Registry, NodeId};
use super::ledger::{Ledger, ComputationError, NodeStatus};
use super::bytecode::{Compiler, Program, OpCode};
use super::kernel;
use std::sync::Arc;

pub struct Engine<'a> {
    registry: &'a Registry,
}

impl<'a> Engine<'a> {
    pub fn new(registry: &'a Registry) -> Self { Self { registry } }

    pub fn compute(&self, targets: &[NodeId], ledger: &mut Ledger) -> Result<(), ComputationError> {
        ledger.ensure_capacity(self.registry.count());
        
        let nodes_to_compute = self.plan(targets, ledger)?;
        
        if nodes_to_compute.is_empty() {
            return Ok(());
        }

        let compiler = Compiler::new(self.registry);
        // Pass ledger for dynamic type resolution
        let program = compiler.compile(&nodes_to_compute, ledger);

        self.execute(&program, ledger)
    }

    fn plan(&self, targets: &[NodeId], ledger: &Ledger) -> Result<Vec<NodeId>, ComputationError> {
        let mut plan = Vec::new();
        let mut state = vec![0u8; self.registry.count()]; 

        for &t in targets {
            self.dfs_plan(t, ledger, &mut plan, &mut state)?;
        }
        Ok(plan)
    }

    fn dfs_plan(&self, node: NodeId, ledger: &Ledger, plan: &mut Vec<NodeId>, state: &mut Vec<u8>) -> Result<(), ComputationError> {
        let idx = node.index();
        
        if ledger.is_computed(node) {
            return Ok(());
        }
        
        if state[idx] == 2 { return Ok(()); }
        if state[idx] == 1 { return Err(ComputationError::CycleDetected); }

        state[idx] = 1;
        for &parent in self.registry.get_parents(node) {
            self.dfs_plan(parent, ledger, plan, state)?;
        }
        state[idx] = 2;
        plan.push(node);
        Ok(())
    }

    fn execute(&self, program: &Program, ledger: &mut Ledger) -> Result<(), ComputationError> {
        for instr in &program.tape {
            let t_idx = instr.target as usize;
            
            match instr.op {
                OpCode::AddScalar => {
                    let l = ledger.scalars[instr.p1 as usize];
                    let r = ledger.scalars[instr.p2 as usize];
                    ledger.scalars[t_idx] = l + r;
                    ledger.status[t_idx] = NodeStatus::ComputedScalar as u8;
                }
                OpCode::SubScalar => {
                    let l = ledger.scalars[instr.p1 as usize];
                    let r = ledger.scalars[instr.p2 as usize];
                    ledger.scalars[t_idx] = l - r;
                    ledger.status[t_idx] = NodeStatus::ComputedScalar as u8;
                }
                OpCode::MulScalar => {
                    let l = ledger.scalars[instr.p1 as usize];
                    let r = ledger.scalars[instr.p2 as usize];
                    ledger.scalars[t_idx] = l * r;
                    ledger.status[t_idx] = NodeStatus::ComputedScalar as u8;
                }
                OpCode::DivScalar => {
                    let l = ledger.scalars[instr.p1 as usize];
                    let r = ledger.scalars[instr.p2 as usize];
                    if r == 0.0 {
                        ledger.set_error(NodeId(instr.target), ComputationError::MathError("Division by zero".into()));
                    } else {
                        ledger.scalars[t_idx] = l / r;
                        ledger.status[t_idx] = NodeStatus::ComputedScalar as u8;
                    }
                }
                OpCode::LoadConstScalar(val) => {
                    ledger.scalars[t_idx] = val;
                    ledger.status[t_idx] = NodeStatus::ComputedScalar as u8;
                }
                OpCode::LoadConstSeries(ptr) => {
                    let vec_ref = &self.registry.constants_data[ptr as usize];
                    ledger.set_series(NodeId(instr.target), Arc::new(vec_ref.clone()));
                }
                OpCode::SolverVar => {
                    ledger.set_scalar(NodeId(instr.target), 0.0);
                }
                _ => {
                    self.execute_fallback(instr, ledger)?;
                }
            }
        }
        Ok(())
    }

    #[inline(never)]
    fn execute_fallback(&self, instr: &super::bytecode::Instruction, ledger: &mut Ledger) -> Result<(), ComputationError> {
        use crate::store::Operation;
        
        let p1_id = NodeId(instr.p1);
        let p2_id = NodeId(instr.p2);
        
        let v1 = ledger.get(p1_id).ok_or(ComputationError::Upstream("Missing p1".into()))??;
        let v2 = ledger.get(p2_id).ok_or(ComputationError::Upstream("Missing p2".into()))??;

        let op = match instr.op {
            OpCode::AddGeneral => Operation::Add,
            OpCode::SubGeneral => Operation::Subtract,
            OpCode::MulGeneral => Operation::Multiply,
            OpCode::DivGeneral => Operation::Divide,
            OpCode::Prev { lag } => Operation::PreviousValue { lag, default_node: p2_id },
            _ => return Err(ComputationError::Mismatch { msg: "Unknown VM Op".into() }),
        };

        let result = kernel::execute(&op, &[&v1, &v2], "VM_Fallback")?;
        ledger.insert(NodeId(instr.target), Ok(result));
        Ok(())
    }
}