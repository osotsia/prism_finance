use crate::store::{Registry, NodeId, NodeKind, Operation};
use super::ledger::ComputationError;
use super::ledger::Ledger; // Added for the new interface

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OpCode {
    Add = 0,
    Sub = 1,
    Mul = 2,
    Div = 3,
    Prev = 4,
    Identity = 5,
}

#[derive(Debug, Clone, Default)]
pub struct Program {
    pub ops: Vec<u8>,
    pub p1: Vec<u32>,
    pub p2: Vec<u32>,
    pub aux: Vec<u32>,

    pub order: Vec<NodeId>,
    pub layout: Vec<u32>,
    pub input_start_index: usize,
}

impl Program {
    /// Translates a logical NodeId to a physical storage index.
    #[inline(always)]
    pub fn physical_index(&self, id: NodeId) -> usize {
        self.layout[id.index()] as usize
    }

    /// Retrieves a value from the ledger using a logical NodeId.
    pub fn get_value<'a>(&self, ledger: &'a Ledger, id: NodeId) -> Option<&'a [f64]> {
        let phys_idx = self.physical_index(id);
        ledger.get_at_index(phys_idx)
    }

    /// Sets a value in the ledger using a logical NodeId.
    pub fn set_value(&self, ledger: &mut Ledger, id: NodeId, value: &[f64]) -> Result<(), ComputationError> {
        let phys_idx = self.physical_index(id);
        ledger.set_input_at_index(phys_idx, value)
    }
}

pub struct Compiler<'a> {
    registry: &'a Registry,
}

impl<'a> Compiler<'a> {
    pub fn new(registry: &'a Registry) -> Self {
        Self { registry }
    }

    pub fn compile(&self, execution_order: Vec<NodeId>) -> Result<Program, ComputationError> {
        let node_count = self.registry.count();
        let mut layout = vec![u32::MAX; node_count];
        
        let mut formulas = Vec::with_capacity(execution_order.len());
        let mut inputs = Vec::new();

        for &node in &execution_order {
            match self.registry.kinds[node.index()] {
                NodeKind::Formula(_) => formulas.push(node),
                _ => inputs.push(node),
            }
        }

        for (i, &node) in formulas.iter().enumerate() {
            layout[node.index()] = i as u32;
        }

        let input_start_index = formulas.len();
        for (i, &node) in inputs.iter().enumerate() {
            layout[node.index()] = (input_start_index + i) as u32;
        }

        let count = formulas.len();
        let mut ops = Vec::with_capacity(count);
        let mut p1 = Vec::with_capacity(count);
        let mut p2 = Vec::with_capacity(count);
        let mut aux = Vec::with_capacity(count);

        for &node in &formulas {
            let kind = &self.registry.kinds[node.index()];
            if let NodeKind::Formula(op) = kind {
                let parents = self.registry.get_parents(node);
                
                let idx1 = parents.get(0).map(|n| layout[n.index()]).unwrap_or(0);
                let idx2 = parents.get(1).map(|n| layout[n.index()]).unwrap_or(0);
                
                let (code, aux_val) = match op {
                    Operation::Add => (OpCode::Add, 0),
                    Operation::Subtract => (OpCode::Sub, 0),
                    Operation::Multiply => (OpCode::Mul, 0),
                    Operation::Divide => (OpCode::Div, 0),
                    Operation::PreviousValue { lag, .. } => (OpCode::Prev, *lag),
                };
                
                ops.push(code as u8);
                p1.push(idx1);
                p2.push(idx2);
                aux.push(aux_val);
            }
        }

        Ok(Program {
            ops,
            p1,
            p2,
            aux,
            order: execution_order,
            layout,
            input_start_index,
        })
    }
}