# The Factory (Compute)

**Role**: Vectorized Numerical Virtual Machine.

This module implements a custom bytecode interpreter designed for high-throughput financial calculations. It segregates the compilation of the execution plan from the runtime execution, allowing for optimizations like SIMD processing and minimal pointer chasing.

## Internal Architecture

### 1. `bytecode.rs` (The Compiler)
*   **Input**: A topologically sorted list of `NodeId`s from the `Registry`.
*   **Output**: A `Program` struct containing a flattened `Vec<Instruction>`.
*   **Instruction Format**: 16-byte fixed-width structs designed to fit in CPU cache lines.
    *   `op`: The operation (Add, Sub, Mul, Div, Prev).
    *   `target`, `p1`, `p2`: 32-bit indices pointing to offsets in the data Ledger.
*   **Logic**: The compiler translates the graph's dependency tree into a linear sequence of imperative operations, stripping away all graph metadata (names, units) effectively "baking" the logic for execution.

### 2. `ledger.rs` (The Memory)
*   **Storage Strategy**: Structure-of-Arrays (SoA).
*   **Layout**: A single contiguous `Vec<f64>` acts as the heap.
    *   Addressing: `BasePointer + (NodeIndex * ModelLength) + TimeStep`.
    *   Capacity: Resized dynamically based on `node_count * model_len`.
*   **Solver Integration**: Contains a dedicated buffer `solver_trace` to store IPOPT convergence history without polluting the calculation memory.

### 3. `engine.rs` (The VM)
*   **Execution Model**: Single-threaded, linear scan of the instruction tape.
*   **Unsafe Access**: Utilizes raw pointer arithmetic (`ptr::add`) to bypass Rust's slice bounds checking during the hot loop.
*   **Responsibility**: It calculates pointers based on the `model_len` and dispatches execution to the `kernel`. It performs no arithmetic itself.

### 4. `kernel.rs` (The ALU)
*   **SIMD Implementation**: Uses the `wide` crate (`f64x4`) to process 4 time-steps per CPU cycle (AVX/Neon).
*   **Hybrid Execution Path**:
    1.  **Scalar Optimization**: If `model_len == 1`, it executes a single f64 operation and returns immediately, bypassing loop setup overhead.
    2.  **Vectorized Loop**: For time-series, it iterates in chunks of 4 (LANE_WIDTH), using unaligned loads/stores.
    3.  **Tail Processing**: Handles remaining elements (mod 4) with scalar fallbacks.
*   **Time-Series Logic (`Prev`)**: Implements memory shifts using `std::ptr::copy_nonoverlapping` to handle temporal lookbacks efficiently.
