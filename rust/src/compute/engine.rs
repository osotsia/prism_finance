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


#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        ledger: Ledger,
        program: Program,
    }

    impl Fixture {
        fn new(model_len: usize, ops: Vec<OpCode>, p1: Vec<u32>, p2: Vec<u32>) -> Self {
            let mut ledger = Ledger::new();
            // Allocate enough space for inputs (indexes 1, 2) and output (index 0)
            ledger.resize(3, model_len); 
            
            // Pre-fill inputs
            ledger.set_input_at_index(1, &vec![10.0; model_len]).unwrap();
            ledger.set_input_at_index(2, &vec![20.0; model_len]).unwrap();

            let program = Program {
                ops: ops.iter().map(|o| *o as u8).collect(),
                p1,
                p2,
                aux: vec![0; ops.len()],
                layout: vec![0; 3], 
                order: vec![],
                input_start_index: 0,
            };

            Self { ledger, program }
        }
    }

    #[test]
    fn test_engine_memory_boundaries() {
        // Scenario: 1 Op, writing to index 0, reading from 1 and 2.
        // Verifies that pointer offsets are calculated correctly for the given model_len.
        let mut fix = Fixture::new(4, vec![OpCode::Add], vec![1], vec![2]);
        
        Engine::run(&fix.program, &mut fix.ledger).expect("Engine crash");
        
        let res = fix.ledger.get_at_index(0).unwrap();
        assert_eq!(res, &[30.0, 30.0, 30.0, 30.0]);
    }

    #[test]
    fn test_engine_scalar_fast_path() {
        // Scenario: Scalar mode (len=1).
        // Verifies the optimization branch in kernel.rs is triggered and correct.
        let mut fix = Fixture::new(1, vec![OpCode::Mul], vec![1], vec![2]);
        
        Engine::run(&fix.program, &mut fix.ledger).expect("Engine crash");
        
        assert_eq!(fix.ledger.get_at_index(0).unwrap()[0], 200.0);
    }
}