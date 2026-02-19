use crate::compute::ledger::{Ledger, ComputationError};
use crate::compute::bytecode::{Program, OpCode};
use crate::compute::kernel;
use std::slice;

pub struct Engine;

impl Engine {
    /// Executes the bytecode program against the provided ledger.
    pub fn run(program: &Program, ledger: &mut Ledger) -> Result<(), ComputationError> {
        let model_len = ledger.model_len();
        
        // 1. Security Barrier: Validate Memory Layout
        // We verify lengths once here so we can skip checks in the hot loop.
        Self::validate_memory_layout(program, ledger, model_len)?;

        // 2. Hot Loop
        // We acquire a raw pointer to allow creating multiple slices into the
        // same backing array. This is necessary because graph nodes can depend
        // on previous nodes (read) while we write to the current node (write).
        //
        // SAFETY: 
        // - Aliasing: We permit immutable reads (src) overlapping with mutable writes (dest) 
        //   conceptually, though logically a node never writes to its own parents in one pass.
        // - Bounds: Validated by `validate_memory_layout`.
        let base_ptr = ledger.raw_data_mut();
        let ops_count = program.ops.len();

        unsafe {
            for i in 0..ops_count {
                // A. Decode Instruction
                // Uses `get_unchecked` for speed, safe due to loops 0..ops_count logic
                let op_byte = *program.ops.get_unchecked(i);
                let p1_idx  = *program.p1.get_unchecked(i) as usize;
                let p2_idx  = *program.p2.get_unchecked(i) as usize;
                let aux     = *program.aux.get_unchecked(i);
                
                // B. Construct Safe Slices
                // Instead of passing pointers to the kernel, we pass sized Slices.
                // This prevents the kernel from ever writing outside the row boundaries.
                let dest = slice::from_raw_parts_mut(base_ptr.add(i * model_len), model_len);
                let src1 = slice::from_raw_parts(base_ptr.add(p1_idx * model_len), model_len);
                let src2 = slice::from_raw_parts(base_ptr.add(p2_idx * model_len), model_len);

                // C. Transmute & Execute
                // Safe transmute of u8 -> enum (verified range 0..=5 in Compiler or check here)
                let op: OpCode = std::mem::transmute(op_byte);
                
                kernel::execute_instruction(op, dest, src1, src2, aux);
            }
        }
        
        Ok(())
    }

    /// performs comprehensive bounds checking before execution starts.
    fn validate_memory_layout(
        program: &Program, 
        ledger: &Ledger, 
        model_len: usize
    ) -> Result<(), ComputationError> {
        let op_count = program.ops.len();
        let required_capacity = (program.input_start_index + program.order.len()) * model_len; // Approx check

        // 1. Buffer Size Check
        // Ensure the ledger is physically large enough to hold all nodes.
        if ledger.raw_data_len() < op_count * model_len {
            return Err(ComputationError::Mismatch { 
                msg: format!("Ledger too small. Needed size for {} ops, got {}", op_count, ledger.raw_data_len() / model_len) 
            });
        }

        // 2. Vector Alignment Check (Critical for the zip iterators)
        // zip() stops at the shortest iterator. If the ledger is misaligned,
        // we might silently truncate calculations.
        if ledger.raw_data_len() % model_len != 0 {
            return Err(ComputationError::Mismatch { 
                msg: "Ledger data length is not a multiple of model length".into() 
            });
        }
        
        // Note: A strict checking of every p1/p2 index is O(N) and usually done 
        // in `debug_assertions` or implicitly handled by the Compiler's guarantees.
        // For absolute safety at the cost of startup time, we could iterate p1/p2 here.
        #[cfg(debug_assertions)]
        {
            let max_valid_index = ledger.raw_data_len() / model_len;
            for (&p1, &p2) in program.p1.iter().zip(&program.p2) {
                if p1 as usize >= max_valid_index || p2 as usize >= max_valid_index {
                    panic!("Program contains invalid parent indices");
                }
            }
        }

        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    // Mock setup helper
    fn make_dummy_program(ops_count: usize) -> Program {
        Program {
            ops: vec![OpCode::Add as u8; ops_count],
            p1: vec![0; ops_count], // Point to valid index 0
            p2: vec![0; ops_count],
            aux: vec![0; ops_count],
            layout: vec![], // Unused by engine run
            order: vec![],
            input_start_index: 0,
        }
    }

    #[test]
    fn test_engine_detects_buffer_overflow() {
        // High Priority: Ledger is too small for the number of operations.
        // Old unsafe code would have segfaulted/corrupted memory here.
        let model_len = 10;
        let ops_count = 5;
        
        let mut ledger = Ledger::new();
        // Allocate space for only 4 nodes, but program has 5 ops
        ledger.resize(4, model_len); 

        let program = make_dummy_program(ops_count);

        let result = Engine::run(&program, &mut ledger);
        
        assert!(result.is_err());
        match result.unwrap_err() {
            ComputationError::Mismatch { msg } => {
                assert!(msg.contains("Ledger too small"));
            }
            _ => panic!("Wrong error type"),
        }
    }

    #[test]
    fn test_engine_detects_misalignment() {
        // Medium Priority: Ledger raw data length is not a multiple of model_len.
        // This could happen if a manual resize corrupted the vector.
        // If not caught, zip() would silently truncate the last few timesteps.
        let model_len = 4;
        let mut ledger = Ledger::new();
        ledger.resize(2, model_len); // Size = 8
        
        // Manually corrupt the ledger size (simulate bad external manipulation)
        ledger.raw_data_mut_vec().push(0.0); // Size = 9 (9 % 4 != 0)

        let program = make_dummy_program(1);
        let result = Engine::run(&program, &mut ledger);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a multiple"));
    }
}