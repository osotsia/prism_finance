# The Receptionist (Bindings)

**Role**: FFI Interface and State Management.

This module implements the `prism_finance._core` Python extension using PyO3. It manages the lifecycle of the graph and translates between Python's dynamic typing and Rust's static memory models.

## Key Mechanisms

### 1. JIT Compilation & Caching (`python.rs`)
The `PyComputationGraph` struct maintains a `cached_program: Option<Program>`.
*   **Invalidation**: Any method that mutates the graph topology sets the cache to `None`.
*   **Lazy Compilation**: `compute()` and `solve()` check the cache. If `None`, they trigger a topological sort (DFS) and compilation pass before execution.
*   **Address Translation**: The `Compiler` generates a `layout` map translating **Logical Node IDs** (Registry index) to **Physical Storage Indices** (Ledger offset). The Python binding layer uses this map to read/write values to the correct location in the linearized Ledger.

### 2. Data Marshaling
*   **Input**: Python lists are converted to Rust `Vec<f64>`.
*   **Output**: The `PyComputationGraph.get_value` method handles the lookup of physical indices to return data to Python.
*   **Scalar Unwrapping**: The system implements a recursive check (`check_is_scalar`) to determine if a node is structurally a scalar (constant or derived purely from constants) vs. a time-series.

### 3. Isolated Benchmarking
*   **`benchmark_pure_rust`**: An exported function that generates a random graph and runs the engine entirely within Rust. It includes the overhead of translating Logical IDs to Physical Indices during input loading to provide a realistic performance profile.

### 4. Error Mapping
Translates internal Rust errors into Python exceptions:
*   `ComputationError` -> `RuntimeError` (e.g., solver failure).
*   `ValidationError` -> `ValueError` (e.g., unit mismatch, temporal inconsistency).