# The Receptionist (Bindings)

**Role**: FFI Interface and State Management.

This module implements the `prism_finance._core` Python extension using PyO3. It manages the lifecycle of the graph and translates between Python's dynamic typing and Rust's static memory models.

## Key Mechanisms

### 1. JIT Compilation & Caching (`python.rs`)
The `PyComputationGraph` struct maintains a `cached_program: Option<Program>`.
*   **Invalidation**: Any method that mutates the graph topology (e.g., `add_constant_node`, `must_equal`, `update_constant_node`) sets the cache to `None`.
*   **Lazy Compilation**: `compute()` and `solve()` check the cache. If `None`, they trigger a topological sort and compilation pass before execution.
*   **Incremental Compilation**: When `recompute(changed_inputs)` is called, the module calculates the "dirty set" (downstream dependencies) and compiles a *partial* program containing only the instructions necessary to update the affected nodes.

### 2. Data Marshaling
*   **Input**: Python lists are converted to Rust `Vec<f64>`.
*   **Output**: The `PyLedger` wrapper exposes `get_value`.
*   **Scalar Unwrapping**: The system implements a recursive check (`check_is_scalar`) to determine if a node is structurally a scalar (constant or derived purely from constants) vs. a time-series. This allows the API to return a `float` instead of `[float]` when appropriate, matching user expectations.

### 3. Isolated Benchmarking
*   **`benchmark_pure_rust`**: An exported function that generates a random graph and runs the engine entirely within Rust. This is used to profile the `kernel` and `engine` performance without the noise of Python interpreter overhead or FFI context switching.

### 4. Error Mapping
Translates internal Rust errors into Python exceptions:
*   `ComputationError` -> `RuntimeError` (e.g., solver failure, cycle detected during compute).
*   `ValidationError` -> `ValueError` (e.g., unit mismatch, temporal inconsistency).
