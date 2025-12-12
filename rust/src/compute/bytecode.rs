use crate::store::{Registry, NodeId, NodeKind, Operation};
use super::ledger::{NodeStatus, Ledger};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpCode {
    AddScalar,
    SubScalar,
    MulScalar,
    DivScalar,
    AddGeneral,
    SubGeneral,
    MulGeneral,
    DivGeneral,
    LoadConstScalar(f64),
    LoadConstSeries(u32),
    Prev { lag: u32 },
    SolverVar,
}

#[derive(Debug, Clone)]
pub struct Instruction {
    pub op: OpCode,
    pub target: u32,
    pub p1: u32,
    pub p2: u32,
}

#[derive(Debug, Default)]
pub struct Program {
    pub tape: Vec<Instruction>,
}

#[derive(Clone, Copy, PartialEq)]
enum TypeHint {
    Unknown,
    Scalar,
    Series,
}

pub struct Compiler<'a> {
    registry: &'a Registry,
}

impl<'a> Compiler<'a> {
    pub fn new(registry: &'a Registry) -> Self {
        Self { registry }
    }

    /// Compiles with dynamic type resolution based on the provided ledger.
    pub fn compile(&self, sorted_nodes: &[NodeId], ledger: &Ledger) -> Program {
        let mut tape = Vec::with_capacity(sorted_nodes.len());
        let mut type_map = vec![TypeHint::Unknown; self.registry.count()];

        for &node in sorted_nodes {
            let idx = node.index();
            let kind = &self.registry.kinds[idx];
            let target = node.0;

            let (instr, inferred_type) = match kind {
                NodeKind::Scalar(val) => (
                    Instruction { op: OpCode::LoadConstScalar(*val), target, p1: 0, p2: 0 },
                    TypeHint::Scalar
                ),
                NodeKind::TimeSeries(ptr) => (
                    Instruction { op: OpCode::LoadConstSeries(*ptr), target, p1: 0, p2: 0 },
                    TypeHint::Series
                ),
                NodeKind::SolverVariable => (
                    Instruction { op: OpCode::SolverVar, target, p1: 0, p2: 0 },
                    TypeHint::Scalar 
                ),
                NodeKind::Formula(op) => {
                    let parents = self.registry.get_parents(node);
                    let p1 = parents.get(0).map(|n| n.index()).unwrap_or(0);
                    let p2 = parents.get(1).map(|n| n.index()).unwrap_or(0);

                    // Resolve with Ledger awareness
                    let t1 = self.resolve_type(p1, &type_map, ledger);
                    let t2 = self.resolve_type(p2, &type_map, ledger);
                    
                    let is_scalar_op = t1 == TypeHint::Scalar && t2 == TypeHint::Scalar;
                    let result_type = if is_scalar_op { TypeHint::Scalar } else { TypeHint::Series };

                    let opcode = match op {
                        Operation::Add => if is_scalar_op { OpCode::AddScalar } else { OpCode::AddGeneral },
                        Operation::Subtract => if is_scalar_op { OpCode::SubScalar } else { OpCode::SubGeneral },
                        Operation::Multiply => if is_scalar_op { OpCode::MulScalar } else { OpCode::MulGeneral },
                        Operation::Divide => if is_scalar_op { OpCode::DivScalar } else { OpCode::DivGeneral },
                        
                        Operation::PreviousValue { lag, .. } => OpCode::Prev { lag: *lag },
                    };

                    (
                        Instruction { op: opcode, target, p1: p1 as u32, p2: p2 as u32 },
                        result_type
                    )
                }
            };
            
            tape.push(instr);
            type_map[idx] = inferred_type;
        }

        Program { tape }
    }

    fn resolve_type(&self, idx: usize, type_map: &[TypeHint], ledger: &Ledger) -> TypeHint {
        if type_map[idx] != TypeHint::Unknown {
            return type_map[idx];
        }
        
        // Dynamic Check: If the value exists in the ledger, trust its format.
        // This is crucial for SolverVariables which might be Series or Scalar depending on context.
        if let Some(&status) = ledger.status.get(idx) {
             if status == NodeStatus::ComputedScalar as u8 { return TypeHint::Scalar; }
             if status == NodeStatus::ComputedSeries as u8 { return TypeHint::Series; }
        }
        
        // Static Fallback
        match &self.registry.kinds[idx] {
            NodeKind::Scalar(_) => TypeHint::Scalar,
            NodeKind::TimeSeries(_) => TypeHint::Series,
            NodeKind::SolverVariable => TypeHint::Scalar, // Default if not in ledger
            NodeKind::Formula(_) => TypeHint::Series, // Conservative fallback
        }
    }
}