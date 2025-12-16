use crate::compute::ledger::{Ledger, ComputationError};
use crate::compute::bytecode::{Program, OpCode};
use crate::compute::kernel;

pub struct Engine;

impl Engine {
    pub fn run(program: &Program, ledger: &mut Ledger) -> Result<(), ComputationError> {
        let model_len = ledger.model_len();
        let base_ptr = ledger.raw_data_mut();
        let count = program.ops.len();

        // Safety:
        // 1. Compiler ensures p1/p2 indices are < ledger.size().
        // 2. We iterate 0..count. Since we allocated storage for all nodes, 
        //    and count is num_formulas, dest_ptr is always valid.
        unsafe {
            for i in 0..count {
                // Optimization: Strictly sequential write target.
                // CPU can prefetch cache lines and merge stores efficiently.
                let dest_ptr = base_ptr.add(i * model_len);
                
                let p1_idx = *program.p1.get_unchecked(i);
                let p2_idx = *program.p2.get_unchecked(i);
                
                let p1_ptr = base_ptr.add(p1_idx as usize * model_len);
                let p2_ptr = base_ptr.add(p2_idx as usize * model_len);
                
                // OpCode is u8, transmute to enum (safe because values are 0-5)
                let op: OpCode = std::mem::transmute(*program.ops.get_unchecked(i));
                let aux = *program.aux.get_unchecked(i);

                kernel::execute_instruction(
                    op,
                    model_len,
                    dest_ptr,
                    p1_ptr,
                    p2_ptr,
                    aux
                );
            }
        }
        
        Ok(())
    }
}