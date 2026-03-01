use crate::store::{Registry, NodeId, NodeKind, Operation};
use super::ledger::ComputationError;
use super::ledger::Ledger;

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
    // Structure-of-Arrays (SoA) Bytecode
    // -----------------------------------
    pub ops: Vec<u8>,
    pub p1: Vec<u32>,
    pub p2: Vec<u32>,
    pub aux: Vec<u32>,

    /// Maps Logical NodeId -> Physical Storage Index.
    /// Used by the API to read/write values by NodeId.
    pub layout: Vec<u32>,
    
    /// The physical index where Input nodes begin.
    /// Memory Layout: [Formula Results (0..N)] [Inputs (N..M)]
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
        if id.index() >= self.layout.len() { return None; }
        let phys_idx = self.physical_index(id);
        ledger.get_at_index(phys_idx)
    }

    /// Sets a value in the ledger using a logical NodeId.
    pub fn set_value(&self, ledger: &mut Ledger, id: NodeId, value: &[f64]) -> Result<(), ComputationError> {
        if id.index() >= self.layout.len() { 
             return Err(ComputationError::Mismatch { msg: "Node ID out of bounds".into() });
        }
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

    /// Compiles the Registry into a linearized execution plan.
    ///
    /// Strategy: Segregated Storage with First-Use Ordering
    /// 1. Formulas: Assigned slots 0..N (Topological Order).
    ///    - Ensures strictly sequential writes for Write Combining.
    /// 2. Inputs: Assigned slots N..Total (First-Use Order).
    ///    - Ensures strictly sequential reads for Hardware Prefetching.
    pub fn compile(&self, topological_sort: Vec<NodeId>) -> Result<Program, ComputationError> {
        let node_count = self.registry.count();
        let mut layout = vec![u32::MAX; node_count];
        
        // 1. Identify Formulas
        // Extract formulas from the topological sort to form the instruction stream.
        let mut formula_nodes = Vec::with_capacity(topological_sort.len());
        for &node in &topological_sort {
            if matches!(self.registry.kinds[node.index()], NodeKind::Formula(_)) {
                formula_nodes.push(node);
            }
        }

        // 2. Order Inputs by First Use
        // Scan the formula sequence to find which inputs are needed and in what order.
        let mut ordered_inputs = Vec::new();
        let mut input_seen = vec![false; node_count];

        // A. Collect utilized inputs
        for &node in &formula_nodes {
             let parents = self.registry.get_parents(node);
             for &p in parents {
                 // If parent is NOT a formula, it is an Input (Scalar/Series/SolverVar).
                 if !matches!(self.registry.kinds[p.index()], NodeKind::Formula(_)) {
                     if !input_seen[p.index()] {
                         input_seen[p.index()] = true;
                         ordered_inputs.push(p);
                     }
                 }
             }
        }

        // B. Collect orphans (Inputs defined but never used)
        // We scan 0..node_count to catch any inputs not marked as seen.
        for i in 0..node_count {
            if !matches!(self.registry.kinds[i], NodeKind::Formula(_)) {
                if !input_seen[i] {
                    ordered_inputs.push(NodeId::new(i));
                }
            }
        }

        // 3. Assign Physical Slots
        // Formulas: 0 .. N
        for (i, &node) in formula_nodes.iter().enumerate() {
            layout[node.index()] = i as u32;
        }

        // Inputs: N .. Total
        let input_start_index = formula_nodes.len();
        for (i, &node) in ordered_inputs.iter().enumerate() {
            layout[node.index()] = (input_start_index + i) as u32;
        }

        // 4. Generate Bytecode
        let count = formula_nodes.len();
        let mut ops = Vec::with_capacity(count);
        let mut p1 = Vec::with_capacity(count);
        let mut p2 = Vec::with_capacity(count);
        let mut aux = Vec::with_capacity(count);

        for &node in &formula_nodes {
            let kind = &self.registry.kinds[node.index()];
            if let NodeKind::Formula(op) = kind {
                let parents = self.registry.get_parents(node);
                
                // Resolve parent Logical IDs to Physical Indices using the new layout
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
            layout,
            input_start_index,
        })
    }
}