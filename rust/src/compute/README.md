# The Factory (Compute)

**Role**: Numerical Execution.

This module turns a `Registry` (Recipe) and inputs into a `Ledger` (Result).

## Components

### 1. `engine.rs` (The Manager)
*   Orchestrates the execution loop.
*   Determines the execution plan (Single-threaded DFS for now, extensible to Rayon/Parallel).
*   Manages access to the Ledger.

### 2. `kernel.rs` (The Machine)
*   **Pure Functions Only**: `fn(Op, &[Input]) -> Output`.
*   **Optimization Sandbox**: This file has zero dependency on the Graph structure. It operates purely on `Value` enums.
*   **SIMD Target**: This is where AVX/Neon intrinsics should be applied.

### 3. `ledger.rs` (The Inventory)
*   Stores the results of computations.
*   Handles `Value` variants (Scalars vs TimeSeries vectors).