use crate::compute::bytecode::OpCode;
use wide::f64x4;

// --- Configuration ---
type SimdType = f64x4;
const LANE_WIDTH: usize = 4;

/// Executes a single instruction.
/// Uses SIMD for arithmetic and `copy_nonoverlapping` for shifts.
#[inline(always)]
pub unsafe fn execute_instruction(
    op: OpCode,
    len: usize,
    dest: *mut f64,
    src1: *const f64,
    src2: *const f64,
) {
    match op {
        OpCode::Add => apply_arithmetic(len, dest, src1, src2, |a, b| a + b, |a, b| a + b),
        OpCode::Sub => apply_arithmetic(len, dest, src1, src2, |a, b| a - b, |a, b| a - b),
        OpCode::Mul => apply_arithmetic(len, dest, src1, src2, |a, b| a * b, |a, b| a * b),
        OpCode::Div => apply_arithmetic(len, dest, src1, src2, |a, b| a / b, |a, b| a / b),
        OpCode::Prev { lag } => apply_shift(len, dest, src1, src2, lag as usize),
        OpCode::Identity => {}
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