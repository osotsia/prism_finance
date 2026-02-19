use crate::compute::bytecode::OpCode;

/// Executes a single mathematical operation over a time-series vector.
///
/// # Safety
/// This function is safe. It relies on Rust slices to enforce boundaries.
/// Performance relies on the compiler auto-vectorizing the Zip iterators.
#[inline(always)]
pub fn execute_instruction(
    op: OpCode,
    dest: &mut [f64],
    src1: &[f64],
    src2: &[f64],
    aux: u32,
) {
    // Optimization: The compiler removes bounds checks because zip
    // halts at the shortest slice. Since we ensure slices are equal length
    // in the Engine, this loop runs without conditionals.
    match op {
        OpCode::Add => {
            for ((d, a), b) in dest.iter_mut().zip(src1).zip(src2) {
                *d = *a + *b;
            }
        }
        OpCode::Sub => {
            for ((d, a), b) in dest.iter_mut().zip(src1).zip(src2) {
                *d = *a - *b;
            }
        }
        OpCode::Mul => {
            for ((d, a), b) in dest.iter_mut().zip(src1).zip(src2) {
                *d = *a * *b;
            }
        }
        OpCode::Div => {
            for ((d, a), b) in dest.iter_mut().zip(src1).zip(src2) {
                *d = *a / *b;
            }
        }
        OpCode::Prev => apply_shift(dest, src1, src2, aux as usize),
        OpCode::Identity => { /* No-op */ }
    }
}

/// Handles temporal shifts (e.g., "Previous Value").
///
/// Logic:
/// 1. Fill the 'gap' created by the lag with the default value (src2).
/// 2. Copy the remaining history from the main value (src1).
#[inline(always)]
fn apply_shift(dest: &mut [f64], src_main: &[f64], src_default: &[f64], lag: usize) {
    let len = dest.len();

    if lag >= len {
        // Entire period is covered by default (lag is huge)
        dest.copy_from_slice(src_default);
    } else {
        // 1. The Gap: Fill start of dest with default values
        let (dest_gap, dest_rest) = dest.split_at_mut(lag);
        dest_gap.copy_from_slice(&src_default[0..lag]);

        // 2. The Shift: Copy history into the rest
        // We take the main source, chop off the end (which falls off the timeline),
        // and copy it to the rest of dest.
        let copy_len = len - lag;
        dest_rest.copy_from_slice(&src_main[0..copy_len]);
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kernel_zip_truncation_safety() {
        // High Priority: Verify that if slices are somehow mismatched in length,
        // the kernel does not panic or read out of bounds (zip behavior).
        // While Engine prevents this, the Kernel unit must be robust.
        let mut dest = vec![0.0; 3]; // Shortest
        let src1 = vec![1.0; 4];
        let src2 = vec![2.0; 5];
        
        execute_instruction(OpCode::Add, &mut dest, &src1, &src2, 0);
        
        assert_eq!(dest, vec![3.0, 3.0, 3.0]); // 1+2
    }

    #[test]
    fn test_kernel_temporal_shift_overflow() {
        // Medium Priority: Verify logic when lag > model length.
        // This hits the `if lag >= len` branch to prevent split_at_mut panic.
        let model_len = 5;
        let main_data = vec![10.0; model_len];
        let default_data = vec![99.0; model_len];
        let mut dest = vec![0.0; model_len];

        // Lag = 10 (exceeds length 5)
        apply_shift(&mut dest, &main_data, &default_data, 10);
        
        // Should be all defaults
        assert_eq!(dest, vec![99.0, 99.0, 99.0, 99.0, 99.0]);
    }

    #[test]
    fn test_kernel_aliasing_correctness() {
        // High Priority: Verify calculations work when Dest and Source are the same slice.
        // A = A + B (Accumulator pattern)
        let mut data_a = vec![10.0, 10.0, 10.0];
        let data_b = vec![5.0, 5.0, 5.0];

        // Unsafe block required only to create aliased mutable/immutable refs for the test setup
        // The kernel itself handles the operation safely via Copy semantics of f64.
        let ptr = data_a.as_mut_ptr();
        unsafe {
            let dest = std::slice::from_raw_parts_mut(ptr, 3);
            let src1 = std::slice::from_raw_parts(ptr, 3);
            execute_instruction(OpCode::Add, dest, src1, &data_b, 0);
        }

        assert_eq!(data_a, vec![15.0, 15.0, 15.0]);
    }
}