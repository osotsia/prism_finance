use crate::compute::ledger::{Ledger, ComputationError};
use crate::compute::bytecode::{Program, OpCode};
use crate::compute::kernel;

pub struct Engine;

impl Engine {
    pub fn run(program: &Program, ledger: &mut Ledger) -> Result<(), ComputationError> {
        let model_len = ledger.model_len();
        let base_ptr = ledger.raw_data_mut();
        let count = program.ops.len();
        
        // Debug Safety Check: Ensure the bytecode implies valid memory access
        // This compiles out in release builds but protects tests.
        #[cfg(debug_assertions)]
        {
            let total_size = ledger.model_len() * program.layout.len(); // approx check
            // A more rigorous check would verify every p1/p2 against total_size
            // but for O(1) checks, we verify basic pointer validity below.
        }

        unsafe {
            for i in 0..count {
                let dest_offset = i * model_len;
                let dest_ptr = base_ptr.add(dest_offset);
                
                let p1_idx = *program.p1.get_unchecked(i);
                let p2_idx = *program.p2.get_unchecked(i);
                
                let p1_offset = p1_idx as usize * model_len;
                let p2_offset = p2_idx as usize * model_len;

                // SAFETY ASSERTIONS (Debug only)
                #[cfg(debug_assertions)]
                {
                    // Ensure we are not pointing outside the allocated ledger vector
                    // We calculate the end pointer of the requested slice
                    let ledger_len = ledger.raw_data_len(); 
                    assert!(dest_offset + model_len <= ledger_len, "Engine Segfault Risk: Dest Ptr OOB");
                    assert!(p1_offset + model_len <= ledger_len, "Engine Segfault Risk: P1 Ptr OOB");
                    assert!(p2_offset + model_len <= ledger_len, "Engine Segfault Risk: P2 Ptr OOB");
                }

                let p1_ptr = base_ptr.add(p1_offset);
                let p2_ptr = base_ptr.add(p2_offset);
                
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