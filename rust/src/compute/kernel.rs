use crate::compute::bytecode::OpCode;
use wide::f64x4;

// --- Configuration ---
type SimdType = f64x4;
const LANE_WIDTH: usize = 4;

/// Executes a single instruction.
///
/// **Params:**
/// - `aux`: Auxiliary data (e.g., lag for Prev).
#[inline(always)]
pub unsafe fn execute_instruction(
    op: OpCode,
    len: usize,
    dest: *mut f64,
    src1: *const f64,
    src2: *const f64,
    aux: u32,
) {
    match op {
        OpCode::Add => apply_arithmetic(len, dest, src1, src2, |a, b| a + b, |a, b| a + b),
        OpCode::Sub => apply_arithmetic(len, dest, src1, src2, |a, b| a - b, |a, b| a - b),
        OpCode::Mul => apply_arithmetic(len, dest, src1, src2, |a, b| a * b, |a, b| a * b),
        OpCode::Div => apply_arithmetic(len, dest, src1, src2, |a, b| a / b, |a, b| a / b),
        OpCode::Prev => apply_shift(len, dest, src1, src2, aux as usize),
        OpCode::Identity => {} // No-op
    }
}

/// Generic driver for arithmetic operations.
/// 
/// Accepts two closures to allow the compiler to inline specific operations:
/// 1. `simd_op`: Operations on 256-bit vectors (f64x4).
/// 2. `scalar_op`: Fallback operations for the tail end (f64).
#[inline(always)]
unsafe fn apply_arithmetic<S, C>(
    len: usize,
    dest: *mut f64,
    src1: *const f64,
    src2: *const f64,
    simd_op: S,
    scalar_op: C,
) where 
    S: Fn(SimdType, SimdType) -> SimdType,
    C: Fn(f64, f64) -> f64,
{
    // Optimization: Hot path for Scalar models (len=1).
    // This avoids loop setup overhead, which is critical for the pure_rust_benchmark.
    if len == 1 {
        *dest = scalar_op(*src1, *src2);
        return;
    }

    let mut i = 0;

    // 1. Chunk Phase (SIMD)
    // Only enter if we have at least one full vector width.
    if len >= LANE_WIDTH {
        while i + LANE_WIDTH <= len {
            // Unaligned loads/stores are necessary as Ledger nodes are packed tightly.
            // Casting to [f64; 4] ensures we use safe standard library methods for the memory access.
            let arr_a = src1.add(i).cast::<[f64; 4]>().read_unaligned();
            let arr_b = src2.add(i).cast::<[f64; 4]>().read_unaligned();
            
            let a = SimdType::from(arr_a);
            let b = SimdType::from(arr_b);
            
            let res = simd_op(a, b);
            
            let arr_res = res.to_array();
            dest.add(i).cast::<[f64; 4]>().write_unaligned(arr_res);
            
            i += LANE_WIDTH;
        }
    }

    // 2. Tail Phase (Scalar)
    // Handle remaining elements (or small vectors where len < 4).
    while i < len {
        let a = *src1.add(i);
        let b = *src2.add(i);
        *dest.add(i) = scalar_op(a, b);
        i += 1;
    }
}

/// Optimized memory move for time-series shifts.
#[inline(always)]
unsafe fn apply_shift(
    len: usize,
    dest: *mut f64,
    src_main: *const f64,
    src_default: *const f64,
    lag: usize,
) {
    if lag >= len {
        // Shift exceeds timeline; entire result is default.
        std::ptr::copy_nonoverlapping(src_default, dest, len);
    } else {
        // 1. Fill gap with default
        std::ptr::copy_nonoverlapping(src_default, dest, lag);
        // 2. Copy shifted main data
        std::ptr::copy_nonoverlapping(src_main, dest.add(lag), len - lag);
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    // --- Helpers ---
    
    /// Runs a specific op over a range of vector lengths to catch
    /// off-by-one errors in the SIMD/Scalar transition logic.
    fn verify_op_parity(op: OpCode, expected_val: f64) {
        // Test 0 (empty), 1 (scalar fast-path), 4 (exact SIMD lane), 
        // 5 (SIMD + 1 tail), 17 (multiple SIMD + tail)
        for len in 0..=17 {
            let a = vec![2.0; len];
            let b = vec![4.0; len];
            let mut dest = vec![0.0; len];
            let aux = 0;

            unsafe {
                execute_instruction(op, len, dest.as_mut_ptr(), a.as_ptr(), b.as_ptr(), aux);
            }

            for (i, val) in dest.iter().enumerate() {
                assert_eq!(*val, expected_val, "Failed at len {}, index {}", len, i);
            }
        }
    }

    // --- Tests ---

    #[test]
    fn test_arithmetic_kernels() {
        verify_op_parity(OpCode::Add, 6.0); // 2 + 4
        verify_op_parity(OpCode::Mul, 8.0); // 2 * 4
        verify_op_parity(OpCode::Div, 0.5); // 2 / 4
    }

    #[test]
    fn test_prev_shift_logic() {
        // Verify time-series shifting across boundary conditions
        let max_len = 10;
        let main: Vec<f64> = (0..max_len).map(|v| v as f64).collect();
        let default = vec![-1.0; max_len];

        // Case 1: Standard Shift (Lag 2) -> [-1, -1, 0, 1, 2...]
        let mut dest = vec![0.0; max_len];
        unsafe {
            execute_instruction(OpCode::Prev, max_len, dest.as_mut_ptr(), main.as_ptr(), default.as_ptr(), 2);
        }
        assert_eq!(dest[0], -1.0);
        assert_eq!(dest[2], 0.0);

        // Case 2: Lag > Len -> All Default
        unsafe {
            execute_instruction(OpCode::Prev, max_len, dest.as_mut_ptr(), main.as_ptr(), default.as_ptr(), 20);
        }
        assert!(dest.iter().all(|&x| x == -1.0));

        // Case 3: Identity (Lag 0) -> Exact Copy
        unsafe {
            execute_instruction(OpCode::Prev, max_len, dest.as_mut_ptr(), main.as_ptr(), default.as_ptr(), 0);
        }
        assert_eq!(dest, main);
    }
}