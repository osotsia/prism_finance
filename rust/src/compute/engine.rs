use crate::compute::ledger::{Ledger, ComputationError};
use crate::compute::bytecode::Program;
use crate::compute::kernel;

pub struct Engine;

impl Engine {
    pub fn run(program: &Program, ledger: &mut Ledger) -> Result<(), ComputationError> {
        let model_len = ledger.model_len();
        let base_ptr = ledger.raw_data_mut();
        
        // Safety: The compiler guarantees indices < node_capacity.
        unsafe {
            for instr in &program.tape {
                // Cast u32 -> usize for pointer offset
                let dest_ptr = base_ptr.add(instr.target as usize * model_len);
                let p1_ptr = base_ptr.add(instr.p1 as usize * model_len);
                let p2_ptr = base_ptr.add(instr.p2 as usize * model_len);
                
                // Pass OpCode directly. It contains u32, kernel casts to usize if needed.
                kernel::execute_instruction(
                    instr.op, 
                    model_len, 
                    dest_ptr, 
                    p1_ptr, 
                    p2_ptr
                );
            }
        }
        
        Ok(())
    }
}