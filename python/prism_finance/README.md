# Python API Layer (`graph.py`)

**Role**: Domain-Specific Language (DSL) and FFI Wrapper.

This module defines the user-facing API for Prism. It implements a declarative syntax using Python context managers and operator overloading, translating high-level Python code into low-level structural mutations on the underlying Rust `_ComputationGraph`.

## Architecture

### 1. `Canvas` (The Container)
*   **Role**: Graph lifecycle and state management.
*   **Context Management**: Uses `contextvars` (`_active_canvas`) to maintain a thread-safe, implicit reference to the active model. This allows `Var` instantiation without explicitly passing the model object (e.g., `a = Var(10)` vs `a = model.add_var(10)`).
*   **Rust Ownership**: Owns the instance of `_core._ComputationGraph` (topology) and `_core._Ledger` (data).
*   **Value Unboxing**: Handles the interface between Rust's vectorized storage and Python's scalar expectations. The `get_value` method queries the Rust engine (`is_scalar`) to determine if a single-element list should be returned as a `float` or kept as a list.

### 2. `Var` (The Handle)
*   **Role**: A lightweight proxy for a graph node.
*   **State**: Contains only the `NodeId` (integer) and a reference to the parent `Canvas`. It holds no data values itself.
*   **Operator Overloading**: Implements `__add__`, `__sub__`, `__mul__`, `__truediv__`.
    *   **Lazy Evaluation**: These operators do *not* perform arithmetic. Instead, they invoke FFI methods on the Rust graph (e.g., `add_binary_formula`) to create new nodes and edges, returning a new `Var` pointing to the result.
*   **Temporal Logic**: The `prev()` method exposes the time-series shift operation, mapping arguments directly to the Rust `Operation::PreviousValue` instruction.

### 3. Execution & Solver Interface
*   **Declarative Constraints**: Methods like `must_equal` do not verify equality immediately. They generate "Residual Nodes" (formulas representing $LHS - RHS$) which are registered with the solver for minimization.
*   **Incremental Updates**: The `recompute(changed_vars)` method accepts a list of Python `Var` objects, extracts their internal IDs, and passes them to the Rust engine to trigger a partial recalculation of the dirty subgraph.

### 4. Static Typing Interface
*   **Metadata Injection**: The `declare_type` method passes user-defined expectations (Units, Stock/Flow) to the Rust registry. This data is inert during computation but is used by the `validate()` routine to perform structural checks.