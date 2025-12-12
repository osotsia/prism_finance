use crate::compute::bytecode::OpCode;

/// Executes a single instruction over the memory slices.
#[inline(always)]
pub unsafe fn execute_instruction(
    op: OpCode,
    len: usize,
    dest: *mut f64,
    src1: *const f64,
    src2: *const f64,
) {
    match op {
        OpCode::Add => {
            for i in 0..len { *dest.add(i) = *src1.add(i) + *src2.add(i); }
        },
        OpCode::Sub => {
            for i in 0..len { *dest.add(i) = *src1.add(i) - *src2.add(i); }
        },
        OpCode::Mul => {
            for i in 0..len { *dest.add(i) = *src1.add(i) * *src2.add(i); }
        },
        OpCode::Div => {
            for i in 0..len { *dest.add(i) = *src1.add(i) / *src2.add(i); }
        },
        OpCode::Prev { lag } => {
            let lag_idx = lag as usize;
            for i in 0..len {
                if i < lag_idx {
                    *dest.add(i) = *src2.add(i);
                } else {
                    *dest.add(i) = *src1.add(i - lag_idx);
                }
            }
        },
        OpCode::Identity => {}
    }
}