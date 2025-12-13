# The Solver

**Role**: Simultaneous Equation Resolution via FFI.

This module integrates the IPOPT (Interior Point OPTimizer) library to resolve circular dependencies and simultaneous equations defined in the graph. It treats the Prism `Engine` as a black-box function evaluator to satisfy constraints of the form $g(x) = 0$.

## Internal Architecture

### 1. `optimizer.rs` (The Orchestrator)
*   **Role**: High-level entry point.
*   **Setup**: Converts the graph's `SolverVariable` nodes into a dense vector of unknowns ($x$) and `Constraint` nodes into a dense vector of residuals ($g(x)$).
*   **Lifecycle**:
    1.  Initializes the `PrismProblem` context.
    2.  Allocates the raw C-compatible IPOPT problem via `ipopt_ffi`.
    3.  Configures tolerances (`1e-9`) and callback pointers.
    4.  Executes the solve.
    5.  Reconstructs a final `Ledger` combining the `base_ledger` (constants) and the solved results.

### 2. `ipopt_adapter.rs` (The Bridge)
*   **Role**: Implements the C-compatible callback functions required by IPOPT.
*   **`eval_g` (Constraint Evaluation)**:
    1.  Receives a candidate vector `x` from IPOPT.
    2.  Writes `x` into the `Ledger`.
    3.  Invokes `Engine::run` to propagate changes through the graph.
    4.  Reads the values of specific "Residual Nodes" (defined as `LHS - RHS`) from the `Ledger` and copies them to IPOPT's buffer.
*   **`eval_jac_g` (Jacobian Evaluation)**:
    *   Since Prism graphs are arbitrary, an analytical Jacobian is not available.
    *   **Finite Differences**: This module implements a numerical Jacobian by perturbing each input variable by $\epsilon$ (`1e-8`) and re-running the `Engine` to measure the rate of change in the residuals.
*   **`eval_f` (Objective Function)**:
    *   Currently returns `0.0`. The solver is configured purely as a feasibility problem (finding roots) rather than minimization.

### 3. `problem.rs` (The Context)
*   **Role**: Holds the state required during the FFI callbacks.
*   **State Management**:
    *   `base_ledger`: A copy of the Ledger containing pre-computed values (constants and independent variables). This is cloned per iteration to ensure a clean state.
    *   `iteration_history`: A `Vec<SolverIteration>` protected by a `Mutex`. This captures convergence metrics (infeasibility, objective value) from the `intermediate_callback` for audit tracing.

### 4. `ipopt_ffi.rs` (The Low-Level)
*   **Role**: Raw `extern "C"` bindings.
*   **Dependencies**: dynamic linking against `libipopt`.
*   **Memory Safety**: Defines the unsafe boundary where Rust pointers are cast to `void*` (`c_void`) to be passed through the C library and cast back in the callbacks.