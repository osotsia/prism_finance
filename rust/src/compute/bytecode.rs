use crate::store::{Registry, NodeId, NodeKind, Operation};
use super::ledger::ComputationError;

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

/// Structure-of-Arrays (SoA) layout for the execution tape.
/// **Optimization:**
/// 1. Reduces memory bandwidth: fetching an op is 1 byte, operands are 4 bytes.
/// 2. target is implicit: Op i always writes to ledger offset i.
#[derive(Debug, Clone, Default)]
pub struct Program {
    // Parallel Arrays
    pub ops: Vec<u8>,
    pub p1: Vec<u32>,
    pub p2: Vec<u32>,
    pub aux: Vec<u32>, // Auxiliary data (e.g., lag for Prev), 0 if unused.

    // Runtime Metadata
    pub order: Vec<NodeId>,        // The topological execution order
    pub layout: Vec<u32>,          // Map: NodeId -> StorageIndex
    pub input_start_index: usize,  // Offset where non-computed inputs begin in Ledger
}

pub struct Compiler<'a> {
    registry: &'a Registry,
}

impl<'a> Compiler<'a> {
    pub fn new(registry: &'a Registry) -> Self {
        Self { registry }
    }

    /// Compiles the graph into a linear, vectorized program.
    ///
    /// **Optimization - Linearization:**
    /// The ledger is re-ordered. 
    /// - Slots 0..N are reserved for the N formulas in the program.
    /// - Instruction i strictly writes to Ledger[i].
    /// - Inputs/Constants are pushed to the end of the Ledger.
    pub fn compile(&self, execution_order: Vec<NodeId>) -> Result<Program, ComputationError> {
        let node_count = self.registry.count();
        let mut layout = vec![u32::MAX; node_count];
        
        // 1. Partition nodes: Formulas (Computed) vs Inputs (Static)
        // We use the topological order to determine the sequence of Formulas.
        let mut formulas = Vec::with_capacity(execution_order.len());
        let mut inputs = Vec::new();

        for &node in &execution_order {
            match self.registry.kinds[node.index()] {
                NodeKind::Formula(_) => formulas.push(node),
                _ => inputs.push(node),
            }
        }

        // 2. Assign Storage Indices
        // Formulas get 0..F (Linear Write Locality)
        for (i, &node) in formulas.iter().enumerate() {
            layout[node.index()] = i as u32;
        }

        let input_start_index = formulas.len();
        // Inputs get F..Total
        for (i, &node) in inputs.iter().enumerate() {
            layout[node.index()] = (input_start_index + i) as u32;
        }

        // 3. Generate Code (SoA)
        let count = formulas.len();
        let mut ops = Vec::with_capacity(count);
        let mut p1 = Vec::with_capacity(count);
        let mut p2 = Vec::with_capacity(count);
        let mut aux = Vec::with_capacity(count);

        for &node in &formulas {
            let kind = &self.registry.kinds[node.index()];
            if let NodeKind::Formula(op) = kind {
                let parents = self.registry.get_parents(node);
                
                // Map parent NodeIds to Storage Indices
                let idx1 = parents.get(0).map(|n| layout[n.index()]).unwrap_or(0);
                let idx2 = parents.get(1).map(|n| layout[n.index()]).unwrap_or(0);
                
                let (code, aux_val) = match op {
                    Operation::Add => (OpCode::Add, 0),
                    Operation::Subtract => (OpCode::Sub, 0),
                    Operation::Multiply => (OpCode::Mul, 0),
                    Operation::Divide => (OpCode::Div, 0),
                    Operation::PreviousValue { lag, default_node } => {
                        // For Prev: p1 is source, p2 is default node
                        let def_idx = layout[default_node.index()];
                        // Override p2 with the remapped default node index
                        // (Note: parents[1] in the graph is the default node, so idx2 is already correct)
                        (OpCode::Prev, *lag)
                    }
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