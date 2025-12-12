use crate::store::{Registry, NodeId, NodeKind, Operation};
use super::ledger::ComputationError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpCode {
    Add, Sub, Mul, Div,
    Prev { lag: u32 }, // u32 to match NodeId width
    Identity, 
}

#[derive(Debug, Clone)]
pub struct Instruction {
    pub op: OpCode,
    // Optimization: Use u32 to reduce struct size from 32 bytes to 16 bytes.
    // This halves the memory bandwidth required to read the program tape.
    pub target: u32,
    pub p1: u32,
    pub p2: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Program {
    pub tape: Vec<Instruction>,
    pub order: Vec<NodeId>, 
}

pub struct Compiler<'a> {
    registry: &'a Registry,
}

impl<'a> Compiler<'a> {
    pub fn new(registry: &'a Registry) -> Self {
        Self { registry }
    }

    pub fn compile(&self, execution_order: Vec<NodeId>) -> Result<Program, ComputationError> {
        let mut tape = Vec::with_capacity(execution_order.len());

        for &node in &execution_order {
            let idx = node.index();
            match &self.registry.kinds[idx] {
                NodeKind::Scalar(_) | NodeKind::TimeSeries(_) | NodeKind::SolverVariable => continue,
                
                NodeKind::Formula(op) => {
                    let parents = self.registry.get_parents(node);
                    let p1 = parents.get(0).map(|n| n.0).unwrap_or(0);
                    let p2 = parents.get(1).map(|n| n.0).unwrap_or(0);
                    let target = node.0; // u32

                    let instr = match op {
                        Operation::Add => Instruction { op: OpCode::Add, target, p1, p2 },
                        Operation::Subtract => Instruction { op: OpCode::Sub, target, p1, p2 },
                        Operation::Multiply => Instruction { op: OpCode::Mul, target, p1, p2 },
                        Operation::Divide => Instruction { op: OpCode::Div, target, p1, p2 },
                        Operation::PreviousValue { lag, default_node } => {
                            Instruction {
                                op: OpCode::Prev { lag: *lag },
                                target,
                                p1, // Source
                                p2: default_node.0, // Default
                            }
                        }
                    };
                    tape.push(instr);
                }
            }
        }

        Ok(Program { tape, order: execution_order })
    }
}