# The Display

**Role**: Visualization & Auditing.

This module is responsible for converting the internal graph state into human-readable strings. It does not perform calculations or modify the graph; it only observes the `Registry` and `Ledger`.

## Key Components

### `trace.rs`
The core engine for generating the "Audit Trace". It performs a recursive depth-first traversal of the dependency graph starting from a target node.

**Features:**
1.  **Equation Reconstruction**: Re-assembles binary operations into readable strings (e.g., `A[10] + B[20]`).
2.  **Solver Inspection**: Detects nodes involved in circular dependencies (Solver Variables) and "explodes" their defining constraints to show the `LHS == RHS` equality proof.
3.  **Deduplication**: Tracks visited constraints to prevent infinite recursion and reduce noise in highly coupled systems.
4.  **Convergence Logging**: Injects IPOPT solver statistics directly into the trace output.