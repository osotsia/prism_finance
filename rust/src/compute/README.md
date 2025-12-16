# The Factory (Compute)

**Role**: Vectorized Numerical Virtual Machine.

This module implements a custom bytecode interpreter designed for high-throughput financial calculations. It segregates the compilation of the execution plan from the runtime execution, allowing for optimizations like SIMD processing and minimal pointer chasing.

## Internal Architecture

### 1. `bytecode.rs` (The Compiler)
*   **Input**: A topologically sorted list of `NodeId`s from the `Registry`.
*   **Output**: A `Program` struct containing parallel vectors (Structure-of-Arrays).
*   **Structure-of-Arrays (SoA)**:
    *   `ops`: `Vec<u8>` (Operation codes)
    *   `p1`, `p2`: `Vec<u32>` (Physical storage indices of operands)
    *   `aux`: `Vec<u32>` (Auxiliary data, e.g., lag for `Prev`)
    *   *Benefit*: Reduces memory bandwidth per instruction from ~24 bytes to 9 bytes.
*   **Linearization**: The compiler re-maps `NodeId`s (Creation Order) to **Storage Indices** (Execution Order). Computed nodes are assigned indices $0 \dots N$, followed by Inputs $N \dots Total$.

### 2. `ledger.rs` (The Memory)
*   **Linearized Storage**: The Ledger does not store data in the order nodes were created. Instead, memory is physically reordered to match the execution sequence.
*   **Storage Strategy**: Structure-of-Arrays (SoA) for time-series data.
    *   Layout: A single contiguous `Vec<f64>`.
    *   Addressing: `PhysicalIndex * ModelLength`.
*   **Write Locality**: Because of linearization, the VM always writes to contiguous memory (`ptr`, `ptr + stride`, ...). This enables perfect hardware prefetching and efficient store-buffer utilization.

### 3. `engine.rs` (The VM)
*   **Implicit Addressing**: The loop does not read a "target" index from the bytecode. Instruction $i$ implicitly writes to Physical Slot $i$.
*   **Execution Model**: Single-threaded, linear scan of the SoA arrays.
*   **Unsafe Access**: Utilizes raw pointer arithmetic (`ptr::add`) to bypass bounds checking during the hot loop.

### 4. `kernel.rs` (The ALU)
*   **SIMD Implementation**: Uses the `wide` crate (`f64x4`) to process 4 time-steps per CPU cycle (AVX/Neon).
*   **Hybrid Execution Path**:
    1.  **Scalar Optimization**: If `model_len == 1`, it executes a single f64 operation and returns immediately, bypassing loop setup overhead.
    2.  **Vectorized Loop**: For time-series, it iterates in chunks of 4 (LANE_WIDTH), using unaligned loads/stores.
*   **Time-Series Logic (`Prev`)**: Implements memory shifts using `std::ptr::copy_nonoverlapping` to handle temporal lookbacks efficiently.