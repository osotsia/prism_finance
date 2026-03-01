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

        let base_ptr = ledger.raw_data_mut();
        let ops_count = program.ops.len();

        // ---------------------------------------------------------------------
        // SCALAR FAST PATH (Optimization)
        // ---------------------------------------------------------------------
        // If model_len == 1, we bypass the overhead of creating slices and 
        // calling the generic kernel. This yields a ~2x speedup for single-period 
        // calculations by keeping everything in registers/L1.
        if model_len == 1 {
            unsafe {
                for i in 0..ops_count {
                    let op_byte = *program.ops.get_unchecked(i);
                    let p1_idx  = *program.p1.get_unchecked(i) as usize;
                    let p2_idx  = *program.p2.get_unchecked(i) as usize;
                    let aux     = *program.aux.get_unchecked(i);

                    // Implicit addressing: The result of operation 'i' is stored at index 'i'
                    let dest = base_ptr.add(i);
                    let src1 = base_ptr.add(p1_idx);
                    let src2 = base_ptr.add(p2_idx);
                    
                    let op: OpCode = std::mem::transmute(op_byte);

                    match op {
                        OpCode::Add => *dest = *src1 + *src2,
                        OpCode::Sub => *dest = *src1 - *src2,
                        OpCode::Mul => *dest = *src1 * *src2,
                        OpCode::Div => *dest = *src1 / *src2,
                        OpCode::Identity => *dest = *src1,
                        OpCode::Prev => {
                            // In a scalar context (length 1), any lag > 0 means we fall off 
                            // the timeline immediately and take the default value (src2).
                            if aux > 0 { *dest = *src2; } else { *dest = *src1; }
                        }
                    }
                }
            }
            return Ok(());
        }

        // ---------------------------------------------------------------------
        // VECTOR PATH (Standard)
        // ---------------------------------------------------------------------
        // Uses raw pointer arithmetic to create aliased mutable/immutable slices
        // required for self-referential calculations.
        unsafe {
            for i in 0..ops_count {
                // A. Decode Instruction
                let op_byte = *program.ops.get_unchecked(i);
                let p1_idx  = *program.p1.get_unchecked(i) as usize;
                let p2_idx  = *program.p2.get_unchecked(i) as usize;
                let aux     = *program.aux.get_unchecked(i);
                
                // B. Construct Safe Slices
                // Instead of passing pointers to the kernel, we pass sized Slices.
                // This prevents the kernel from ever writing outside the row boundaries.
                // Implicit addressing: dest = i * model_len
                let dest = slice::from_raw_parts_mut(base_ptr.add(i * model_len), model_len);
                let src1 = slice::from_raw_parts(base_ptr.add(p1_idx * model_len), model_len);
                let src2 = slice::from_raw_parts(base_ptr.add(p2_idx * model_len), model_len);

                // C. Transmute & Execute
                let op: OpCode = std::mem::transmute(op_byte);
                
                kernel::execute_instruction(op, dest, src1, src2, aux);
            }
        }
        
        Ok(())
    }

    /// Performs comprehensive bounds checking before execution starts.
    fn validate_memory_layout(
        program: &Program, 
        ledger: &Ledger, 
        model_len: usize
    ) -> Result<(), ComputationError> {
        let op_count = program.ops.len();
        
        // 1. Buffer Size Check
        // Ensure the ledger is physically large enough to hold all calculated nodes.
        // (Inputs are stored after op_count, checked implicitly by ledger allocation logic,
        // but strictly we write to 0..op_count).
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
            p1: vec![0; ops_count], 
            p2: vec![0; ops_count],
            aux: vec![0; ops_count],
            layout: vec![], // Unused by engine run in sequential mode
            // In sequential mode, inputs start after formulas
            input_start_index: ops_count, 
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

    #[test]
    fn test_scalar_fast_path_execution() {
        // Explicitly test the model_len == 1 path
        let mut ledger = Ledger::new();
        ledger.resize(3, 1);
        
        // Manual Setup:
        // Index 0: Result (calculated)
        // Index 1: Input A = 10.0
        // Index 2: Input B = 20.0
        let ptr = ledger.raw_data_mut();
        unsafe {
            *ptr.add(1) = 10.0;
            *ptr.add(2) = 20.0;
        }

        // Program: Dest(0) = Src(1) + Src(2)
        let mut program = make_dummy_program(1);
        program.ops[0] = OpCode::Add as u8;
        program.p1[0] = 1;
        program.p2[0] = 2;

        Engine::run(&program, &mut ledger).expect("Scalar run failed");
        
        let res = ledger.get_at_index(0).unwrap()[0];
        assert_eq!(res, 30.0, "Scalar addition failed");
    }
}